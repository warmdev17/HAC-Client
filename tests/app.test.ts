// @vitest-environment jsdom
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { mount, type Status } from '../src/app';

const empty: Status = {
  supported: true, platform: 'windows', configReady: false, profileReady: false,
  keychainReady: false, environmentReady: false, persistenceReady: false,
  codexHome: 'C:\\Users\\Tester\\.codex', issues: [],
};
const tick = async () => { await new Promise(resolve => setTimeout(resolve, 0)); };
const input = () => document.querySelector<HTMLInputElement>('#api-key')!;
const button = (id: string) => document.querySelector<HTMLButtonElement>(`#${id}`)!;
const submit = () => document.querySelector('form')!.dispatchEvent(new Event('submit', { bubbles: true, cancelable: true }));

beforeEach(() => { document.body.innerHTML = '<div id="app"></div>'; });

describe('HAC setup', () => {
  it('disables setup in browser previews without invoking native commands', () => {
    const invoke = vi.fn();
    mount(document.querySelector('#app')!, { desktop: false, invoke });
    expect(button('save').disabled).toBe(true);
    expect(input().disabled).toBe(true);
    expect(invoke).not.toHaveBeenCalled();
    expect(document.body.textContent).toContain('bản xem trước');
  });

  it('shows the Windows storage disclosure and blocks unsupported OS', async () => {
    const invoke = vi.fn().mockResolvedValue(empty);
    mount(document.querySelector('#app')!, { desktop: true, invoke });
    await tick();
    expect(document.querySelector('#os-note')!.textContent).toContain('Registry');
    expect(button('save').disabled).toBe(false);
    invoke.mockResolvedValue({ ...empty, supported: false, platform: 'freebsd' });
    button('refresh').click(); await tick();
    expect(button('save').disabled).toBe(true);
    expect(document.querySelector('#platform')!.textContent).toContain('Chưa hỗ trợ');
  });

  it('clears keys immediately, prevents duplicate submits, preserves success and backup paths', async () => {
    let finish!: (result: unknown) => void;
    const invoke = vi.fn().mockImplementation((command: string) => command === 'get_status'
      ? Promise.resolve(empty)
      : new Promise(resolve => { finish = resolve; }));
    mount(document.querySelector('#app')!, { desktop: true, invoke }); await tick();
    input().value = 'fake-secret'; submit(); submit();
    expect(input().value).toBe('');
    expect(button('save').disabled).toBe(true);
    expect(invoke.mock.calls.filter(([name]) => name === 'setup')).toHaveLength(1);
    expect(invoke).toHaveBeenCalledWith('setup', { request: { apiKey: 'fake-secret' } });
    finish({ backups: ['C:\\backup.toml'], codexHome: empty.codexHome }); await tick();
    expect(document.querySelector('#message')!.textContent).toContain('Đã thiết lập HAC');
    expect(document.querySelector('#message')!.textContent).toContain('chưa xác minh');
    expect(document.querySelector('#backups')!.textContent).toContain('backup.toml');
    expect(document.body.innerHTML).not.toContain('fake-secret');
    expect(button('save').disabled).toBe(false);
  });

  it('clears secrets after failure and displays sanitized native errors', async () => {
    const invoke = vi.fn().mockImplementation((command: string) => command === 'get_status'
      ? Promise.resolve(empty) : Promise.reject('Không lưu được API key. Kiểm tra Credential Manager.'));
    mount(document.querySelector('#app')!, { desktop: true, invoke }); await tick();
    input().value = 'fake-secret'; submit(); await tick();
    expect(input().value).toBe('');
    expect(document.querySelector('#message')!.textContent).toContain('Credential Manager');
    expect(button('save').disabled).toBe(false);
  });

  it('rejects whitespace, allows clearing without native mutations, and toggles key visibility', async () => {
    const invoke = vi.fn().mockResolvedValue(empty);
    mount(document.querySelector('#app')!, { desktop: true, invoke }); await tick();
    input().value = 'key with space'; submit();
    expect(invoke).toHaveBeenCalledTimes(1);
    button('toggle-key').click(); expect(input().type).toBe('text');
    expect(button('toggle-key').getAttribute('aria-label')).toBe('Ẩn API key');
    button('clear').click(); expect(input().value).toBe(''); expect(input().type).toBe('password');
    expect(invoke).toHaveBeenCalledTimes(1);
  });

  it('does not treat environment presence alone as a complete setup or render paths as HTML', async () => {
    const invoke = vi.fn().mockResolvedValue({ ...empty, environmentReady: true, persistenceReady: false, codexHome: '<img src=x onerror=alert(1)>' });
    mount(document.querySelector('#app')!, { desktop: true, invoke }); await tick();
    expect(document.querySelector('#env-state')!.textContent).toBe('Chưa sẵn sàng');
    expect(document.querySelector('#codex-home img')).toBeNull();
    expect(document.querySelector('#codex-home')!.textContent).toContain('<img');
  });
});

it('persists the selected theme and restores it on mount', () => {
  localStorage.clear();
  mount(document.querySelector('#app')!, { desktop: false, invoke: vi.fn() });
  const theme = document.querySelector<HTMLSelectElement>('#theme')!;
  expect(theme.value).toBe('system');
  theme.value = 'dark'; theme.dispatchEvent(new Event('change'));
  expect(document.documentElement.dataset.theme).toBe('dark');
  expect(localStorage.getItem('hac-theme')).toBe('dark');
  mount(document.querySelector('#app')!, { desktop: false, invoke: vi.fn() });
  expect(document.querySelector<HTMLSelectElement>('#theme')!.value).toBe('dark');
  localStorage.clear();
});

it('requires confirmation before removing HAC and clears the entered key', async () => {
  const show = vi.fn(); const close = vi.fn();
  HTMLDialogElement.prototype.showModal = show;
  HTMLDialogElement.prototype.close = close;
  const invoke = vi.fn().mockImplementation((command: string) => Promise.resolve(command === 'uninstall' ? 'Đã gỡ HAC.' : empty));
  mount(document.querySelector('#app')!, { desktop: true, invoke }); await tick();
  input().value = 'fake-secret';
  button('remove').click();
  expect(show).toHaveBeenCalledOnce();
  expect(invoke.mock.calls.filter(([name]) => name === 'uninstall')).toHaveLength(0);
  button('confirm-remove').click(); await tick();
  expect(invoke.mock.calls.filter(([name]) => name === 'uninstall')).toHaveLength(1);
  expect(input().value).toBe('');
  expect(document.querySelector('#message')!.textContent).toBe('Đã gỡ HAC.');
});

it('enables Linux setup and explains Secret Service and session limits', async () => {
  const invoke = vi.fn().mockResolvedValue({ ...empty, platform: 'linux', codexHome: '/home/test/.codex' });
  mount(document.querySelector('#app')!, { desktop: true, invoke }); await tick();
  expect(button('save').disabled).toBe(false);
  expect(button('remove').disabled).toBe(false);
  expect(document.querySelector('#os-note')!.textContent).toContain('Secret Service');
  expect(document.querySelector('#os-note')!.textContent).toContain('systemd');
});

it('confirms reset, clears secrets and reports backups without duplicate requests', async () => {
  HTMLDialogElement.prototype.showModal = function () { this.open = true; };
  HTMLDialogElement.prototype.close = function () { this.open = false; };
  let finish!: (value: unknown) => void;
  const invoke = vi.fn().mockImplementation((command: string) => command === 'get_status'
    ? Promise.resolve(empty) : new Promise(resolve => { finish = resolve; }));
  mount(document.querySelector('#app')!, { desktop: true, invoke }); await tick();
  input().value = 'secret';
  expect(button('reset')).not.toBeNull();
  button('reset').click();
  expect(document.querySelector<HTMLDialogElement>('#reset-dialog')!.open).toBe(true);
  expect(invoke.mock.calls.filter(([name]) => name === 'reset_configs')).toHaveLength(0);
  button('confirm-reset').click(); button('confirm-reset').click();
  expect(input().value).toBe('');
  expect(button('stop-apps').disabled).toBe(true);
  expect(invoke.mock.calls.filter(([name]) => name === 'reset_configs')).toHaveLength(1);
  finish({ backups: ['/tmp/reset-backup'], codexHome: empty.codexHome }); await tick();
  expect(document.querySelector('#backups')!.textContent).toContain('/tmp/reset-backup');
  expect(button('reset').disabled).toBe(false);
});

it('confirms desktop shutdown and recovers controls after a partial failure', async () => {
  HTMLDialogElement.prototype.showModal = function () { this.open = true; };
  HTMLDialogElement.prototype.close = function () { this.open = false; };
  const invoke = vi.fn().mockImplementation((command: string) => command === 'get_status'
    ? Promise.resolve(empty) : Promise.resolve({ stopped: 1, failed: 1 }));
  mount(document.querySelector('#app')!, { desktop: true, invoke }); await tick();
  expect(button('stop-apps')).not.toBeNull();
  button('stop-apps').click();
  expect(invoke.mock.calls.filter(([name]) => name === 'stop_desktop_apps')).toHaveLength(0);
  button('confirm-stop').click(); await tick();
  expect(invoke.mock.calls.filter(([name]) => name === 'stop_desktop_apps')).toHaveLength(1);
  expect(document.querySelector('#feedback')!.classList.contains('error')).toBe(true);
  expect(button('stop-apps').disabled).toBe(false);
  invoke.mockRejectedValue('Không đọc được danh sách tiến trình');
  button('stop-apps').click(); button('confirm-stop').click(); await tick();
  expect(document.querySelector('#message')!.textContent).toContain('Không đọc được');
  expect(button('save').disabled).toBe(false);
});

it('blocks maintenance commands in web previews', () => {
  const invoke = vi.fn();
  mount(document.querySelector('#app')!, { desktop: false, invoke });
  expect(button('reset')).not.toBeNull();
  expect(button('reset').disabled).toBe(true);
  expect(button('stop-apps').disabled).toBe(true);
  button('confirm-reset').click(); button('confirm-stop').click();
  expect(invoke).not.toHaveBeenCalled();
});
