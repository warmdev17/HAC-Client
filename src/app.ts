export type Status = {
  supported: boolean; platform: string; configReady: boolean; profileReady: boolean;
  keychainReady: boolean; environmentReady: boolean; persistenceReady: boolean;
  codexHome: string; issues: string[];
};
type SetupResult = { backups: string[]; codexHome: string };
type Bridge = { invoke: <T>(command: string, args?: Record<string, unknown>) => Promise<T>; desktop: boolean };

const shield = '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" aria-hidden="true"><path d="m12 3 8 3v6c0 4-4 7-8 9-4-2-8-5-8-9V6z"/><path d="m8 12 3 3 5-6"/></svg>';
const eye = '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" aria-hidden="true"><path d="M2 12s4-7 10-7 10 7 10 7-4 7-10 7S2 12 2 12Z"/><circle cx="12" cy="12" r="3"/></svg>';

export function mount(app: HTMLElement, bridge: Bridge) {
  let status: Status | null = null;
  let busy = false;
  app.innerHTML = `
    <main class="shell">
      <header class="topbar"><div class="brand"><span class="brand-mark" aria-hidden="true">H</span><strong>HAC</strong><span class="brand-context">Thiết lập Codex</span></div><span id="platform" class="platform">${bridge.desktop ? 'Đang kiểm tra…' : 'Bản xem trước'}</span></header>
      <section class="intro"><div><h1>Một API key.<br>Codex ở mọi nơi.</h1><p>Kết nối Codex Desktop và CLI với nhà cung cấp HAC.<br class="wide-only"> Thiết lập một lần, sẵn sàng cho những phiên làm việc tiếp theo.</p></div><div class="destinations" aria-label="Dùng cho Codex Desktop và CLI"><div><span class="device" aria-hidden="true">▣</span><span>Codex Desktop</span></div><div><span class="device terminal" aria-hidden="true">&gt;_</span><span>Codex CLI</span></div></div></section>
      <div class="workspace"><section class="setup-panel" aria-labelledby="connection-title">
        <div class="section-heading"><h2 id="connection-title">Kết nối HAC</h2><span class="private-label">${shield} Kho khóa hệ điều hành</span></div>
        <form id="setup-form">
          <label for="api-key">API key</label>
          <div class="key-input"><input id="api-key" name="api-key" type="password" autocomplete="off" autocapitalize="off" spellcheck="false" aria-describedby="key-help" placeholder="Dán API key của bạn" maxlength="4096" required disabled /><button type="button" id="toggle-key" class="icon-button" aria-label="Hiện API key" aria-pressed="false">${eye}</button></div>
          <p id="key-help" class="field-help">Lưu trong Keychain hoặc Credential Manager. Key không được ghi vào file TOML.</p>
          <dl class="connection-details"><div><dt>Provider</dt><dd>HAC <span class="muted">/ Responses API</span></dd></div><div><dt>Base URL</dt><dd><span>Được cấu hình trong bản HAC này</span></dd></div><div><dt>Model</dt><dd><code>gpt-6-astra</code></dd></div><div><dt>Reasoning</dt><dd><span>Mặc định <strong>medium</strong></span><span>Profile hac <strong>high</strong></span></dd></div></dl>
          <div id="os-note" class="os-note">Windows lưu thêm một bản key trong biến môi trường user (Registry). macOS nạp key từ Keychain khi đăng nhập và mở shell.</div>
          <div class="actions"><button class="primary" id="save" type="submit" disabled>Lưu và thiết lập HAC</button><button class="secondary" id="clear" type="button">Xóa ô nhập</button></div>
          <p class="local-note">Chỉ áp dụng trên máy này. Không gửi yêu cầu đến API.</p>
        </form>
      </section><aside class="status-panel" aria-labelledby="status-title">
        <div class="section-heading"><h2 id="status-title">Trạng thái thiết lập</h2><span id="completion" class="completion">—</span></div>
        <ul class="status-list">
          ${[['config', 'Cấu hình mặc định', 'config.toml'], ['profile', 'Profile HAC', 'hac.config.toml'], ['key', 'API key', 'Kho khóa hệ điều hành'], ['env', 'Môi trường', 'HAC_API_KEY và tự nạp']].map(([id, title, detail]) => `<li><span id="${id}-icon" class="status-icon" aria-hidden="true">–</span><div><span>${title}</span><small>${detail}</small></div><span id="${id}-state" class="row-state">Chưa kiểm tra</span></li>`).join('')}
        </ul>
        <button id="refresh" class="secondary refresh" type="button" ${bridge.desktop ? '' : 'disabled'}>Kiểm tra cấu hình</button>
        <div class="location"><span>Thư mục cấu hình</span><code id="codex-home">Chưa xác định</code><small>Tự nhận CODEX_HOME nếu đã đặt.</small></div>
        <details class="details"><summary>Các thay đổi khi thiết lập</summary><p>Sao lưu file cũ cạnh file gốc trước khi cập nhật. Giữ cài đặt không liên quan; thay provider HAC và bỏ profile mặc định cũ để cấu hình mới có hiệu lực.</p><p>Trên macOS: tạo LaunchAgent và thêm đoạn nạp key vào zsh, bash, fish. Trên Windows: đặt biến môi trường cấp người dùng, không cần quyền admin.</p></details>
      </aside></div>
      <section id="feedback" class="feedback" aria-live="polite" aria-atomic="true" hidden><p id="message"></p><ul id="issues"></ul><details id="backup-details" hidden><summary>Xem các bản sao lưu</summary><ul id="backups"></ul></details></section>
      <footer><span>HAC · Công cụ cấu hình độc lập</span><span>Cấu hình cục bộ ≠ kết nối API đã xác minh</span></footer>
    </main>`;

  const el = <T extends HTMLElement>(id: string) => app.querySelector<T>(`#${id}`)!;
  const input = el<HTMLInputElement>('api-key');
  const save = el<HTMLButtonElement>('save');
  const refresh = el<HTMLButtonElement>('refresh');
  const toggle = el<HTMLButtonElement>('toggle-key');

  function controls() {
    const supported = bridge.desktop && status?.supported === true;
    input.disabled = busy || !supported;
    save.disabled = busy || !supported;
    refresh.disabled = busy || !bridge.desktop;
    toggle.disabled = busy || !supported;
    el<HTMLButtonElement>('clear').disabled = busy || !supported;
    save.textContent = busy ? 'Đang xử lý…' : 'Lưu và thiết lập HAC';
    el('setup-form').setAttribute('aria-busy', String(busy));
  }
  function message(text: string, error = false) {
    el('feedback').hidden = false;
    el('feedback').classList.toggle('error', error);
    el('message').textContent = text;
  }
  function displayStatus(next: Status) {
    status = next;
    const labels: Record<string, string> = { macos: 'macOS', windows: 'Windows', linux: 'Linux' };
    const os = labels[next.platform] ?? next.platform;
    el('platform').textContent = next.supported ? os : `${os} · Chưa hỗ trợ`;
    el('codex-home').textContent = next.codexHome;
    el('os-note').textContent = next.platform === 'windows'
      ? 'Để Codex mở độc lập, key được lưu thêm trong biến môi trường user (Registry), ngoài Credential Manager. Đóng và mở lại Codex và terminal sau khi thiết lập; nếu chưa nhận biến, đăng xuất rồi đăng nhập.'
      : next.platform === 'macos'
        ? 'Keychain lưu key; launchctl cấp key cho ứng dụng mới. HAC tạo LaunchAgent và đoạn nạp cho zsh, bash, fish. macOS có thể hỏi quyền truy cập Keychain.'
        : 'Bản này chỉ thiết lập trên macOS và Windows native. Linux và WSL chưa được hỗ trợ.';
    const checks: [string, boolean][] = [['config', next.configReady], ['profile', next.profileReady], ['key', next.keychainReady], ['env', next.environmentReady && next.persistenceReady]];
    checks.forEach(([id, ready]) => {
      el(`${id}-icon`).textContent = ready ? '✓' : '–';
      el(`${id}-icon`).classList.toggle('ok', ready);
      el(`${id}-state`).textContent = ready ? 'Đã xác nhận' : 'Chưa sẵn sàng';
    });
    el('completion').textContent = `${checks.filter(([, ready]) => ready).length}/4`;
    el('issues').replaceChildren(...next.issues.map(issue => { const li = document.createElement('li'); li.textContent = issue; return li; }));
    controls();
  }
  async function readStatus() {
    const next = await bridge.invoke<Status>('get_status');
    displayStatus(next);
    return next;
  }
  function errorText(error: unknown) {
    return typeof error === 'string' ? error : 'Không liên lạc được với HAC. Đóng và mở lại ứng dụng rồi kiểm tra cấu hình.';
  }
  toggle.addEventListener('click', () => {
    const visible = input.type === 'password';
    input.type = visible ? 'text' : 'password';
    toggle.setAttribute('aria-label', visible ? 'Ẩn API key' : 'Hiện API key');
    toggle.setAttribute('aria-pressed', String(visible));
  });
  function clearKey() {
    input.value = ''; input.type = 'password';
    toggle.setAttribute('aria-label', 'Hiện API key'); toggle.setAttribute('aria-pressed', 'false');
  }
  el('clear').addEventListener('click', () => { clearKey(); input.focus(); });
  el('setup-form').addEventListener('submit', async event => {
    event.preventDefault();
    if (busy || !status?.supported || !bridge.desktop) return;
    if (!input.value || /\s/.test(input.value)) { message('Nhập API key không chứa khoảng trắng.', true); input.focus(); return; }
    busy = true; controls();
    el('issues').replaceChildren(); el('backups').replaceChildren(); el('backup-details').hidden = true;
    message('Đang sao lưu, lưu key và thiết lập môi trường…');
    try {
      const pending = bridge.invoke<SetupResult>('setup', { request: { apiKey: input.value } });
      clearKey();
      const result = await pending;
      el('backups').replaceChildren(...result.backups.map(path => { const li = document.createElement('li'); li.textContent = path; return li; }));
      el('backup-details').hidden = result.backups.length === 0;
      message('Đã thiết lập HAC. Đóng hoàn toàn rồi mở lại Codex Desktop và terminal. Cấu hình đã được kiểm tra cục bộ; chưa xác minh kết nối API.');
      try { await readStatus(); } catch { message('Đã lưu cấu hình nhưng chưa đọc lại được trạng thái. Bấm Kiểm tra cấu hình; mở lại Codex và terminal để nhận biến mới.', true); }
    } catch (error) { clearKey(); message(errorText(error), true); }
    finally { busy = false; controls(); }
  });
  refresh.addEventListener('click', async () => {
    if (busy) return;
    busy = true; controls();
    try {
      const next = await readStatus();
      const ready = next.configReady && next.profileReady && next.keychainReady && next.environmentReady && next.persistenceReady;
      message(ready ? 'Cấu hình HAC và môi trường hệ điều hành khớp nhau. Chưa xác minh endpoint thực tế của tiến trình Codex đang chạy.' : 'Kiểm tra xong. Các mục chưa sẵn sàng cần thiết lập lại hoặc xử lý lỗi bên dưới.', !ready);
    } catch (error) { message(errorText(error), true); }
    finally { busy = false; controls(); }
  });
  controls();
  if (!bridge.desktop) {
    message('Đây là bản xem trước giao diện. Mở ứng dụng HAC trên macOS hoặc Windows để lưu key và thiết lập Codex.');
  } else {
    void readStatus().then(next => {
      if (!next.supported) message('Hệ điều hành này chưa được hỗ trợ; HAC chưa thay đổi cấu hình nào.');
      else if (next.issues.length) message('Một số mục chưa kiểm tra được. Xem chi tiết bên dưới.', true);
    }).catch(error => message(errorText(error), true));
  }
}
