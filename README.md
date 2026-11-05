# HAC — Thiết lập Codex

Ứng dụng Tauri 2 + TypeScript cấu hình Codex Desktop và CLI bằng provider HAC trên **macOS, Windows native và Linux desktop x64**. Linux cần Secret Service (GNOME Keyring hoặc KWallet có bật Secret Service), `secret-tool`, `systemctl`, `busctl` và phiên systemd user. Không hỗ trợ WSL trong bản này.

## Chạy và đóng gói

Cần Node.js 24+, Rust stable (tối thiểu 1.90), và công cụ build Tauri của hệ điều hành: Xcode Command Line Tools trên macOS; Visual Studio C++ Build Tools và WebView2 trên Windows.

Trước khi build, sao chép `.env.example` thành `.env` ở thư mục gốc và đặt `HAC_BASE_URL` bằng URL HTTPS thực của provider. `.env` không được commit; không đặt API key vào file này. Biến môi trường `HAC_BASE_URL` có ưu tiên cao hơn `.env`. Thiếu URL hoặc URL không hợp lệ sẽ làm build Rust thất bại. Thay URL cần build lại ứng dụng.

```sh
npm ci
npm run tauri -- dev
```

Đóng gói trên chính hệ điều hành đích:

```sh
npm run tauri -- build
```

Installer nằm trong `src-tauri/target/release/bundle/`. Workflow GitHub Actions xuất ZIP portable cho macOS/Windows và TAR.GZ chứa AppImage cho Linux (không tạo installer), gồm `HAC-Windows-x64-portable`, `HAC-macOS-arm64-portable`, `HAC-macOS-x64-portable` và `HAC-Linux-x64-portable`; tự đính kèm vào GitHub Release khi push tag bắt đầu bằng `v`. Windows giải nén rồi mở `hac-codex-manager.exe` (cần WebView2 Runtime). macOS giải nén rồi mở `HAC.app`; ZIP tạo bằng `ditto` để giữ quyền thực thi và symlink. Artifact GitHub là ZIP bọc ngoài ZIP portable, nên có thể cần giải nén hai lần. Portable vẫn lưu cấu hình/key vào tài khoản người dùng khi bấm thiết lập. Chưa cấu hình chứng chỉ ký code/notarization. Muốn phân phối rộng rãi cần thêm chứng chỉ macOS/Windows vào CI. Bản macOS mặc định theo kiến trúc runner/máy build; dùng target phù hợp nếu cần Intel/Apple Silicon riêng.

`npm run dev` mở bản xem trước web tại `http://127.0.0.1:1420`; chức năng ghi key/cấu hình bị khóa vì không có backend Tauri. Giao diện dùng font hệ thống, không tải font hoặc nội dung từ bên ngoài.

## Linux

Tải `HAC-Linux-x64-portable.tar.gz`, giải nén bằng `tar -xzf`, rồi chạy `./HAC-Linux-x64.AppImage` (hoặc `chmod +x` nếu công cụ giải nén mất quyền thực thi). Bản CI build trên Ubuntu 22.04; không bảo đảm chạy trên distro có glibc cũ hơn.

Ubuntu/Debian: cài `libsecret-tools`, `gnome-keyring` (hoặc Secret Service tương thích) và FUSE 2 (`libfuse2` trên Ubuntu 22.04, `libfuse2t64` trên 24.04). Nếu không dùng FUSE, chạy AppImage với `--appimage-extract-and-run`. Kho khóa phải được mở khóa trong phiên desktop; không lưu key xuống file plaintext khi kho khóa lỗi. Arch: `libsecret`, `gnome-keyring`, `fuse2`.

Build Linux từ source cần thêm GTK/WebKitGTK 4.1, libdbus, librsvg, appindicator và patchelf; danh sách Ubuntu nằm trong workflow. Chạy `npm run tauri -- build --bundles appimage`.

Linux lưu key qua Secret Service, thêm block bash/zsh/fish đọc bằng `secret-tool`, tạo `$XDG_CONFIG_HOME/hac/environment.sh` và `autostart/hac-environment.desktop` (mặc định `~/.config`). Helper nạp key vào `systemd --user` khi đăng nhập; file không chứa key. Thay đổi này chỉ áp dụng cho tiến trình mới được systemd user tạo; không cập nhật môi trường của launcher/ứng dụng đã chạy hay mọi desktop. Với launcher không nhận key, mở Codex từ terminal mới. Nếu kho khóa chưa mở lúc autostart chạy, mở khóa rồi thiết lập lại; luôn mở lại Codex sau đăng nhập. HAC không sửa môi trường kích hoạt D-Bus.

Gỡ HAC xóa credential, bỏ biến trong systemd user và khôi phục các file shell/autostart từ nhật ký. Tiến trình đang chạy vẫn giữ key cũ cho đến khi đóng. Không hỗ trợ WSL, Linux headless hoặc desktop không có systemd user. Kiểm tra trạng thái chỉ xác nhận file, kho khóa và môi trường manager; không xác minh môi trường của mọi ứng dụng.

## Sử dụng

1. Mở HAC trong tài khoản người dùng thông thường.
2. Nhập API key và bấm **Lưu và thiết lập HAC**. Cửa sổ Keychain của macOS có thể yêu cầu quyền.
3. Xem kết quả và đường dẫn bản sao lưu. Đóng hoàn toàn Codex Desktop và terminal, sau đó mở lại. Không tự đóng ứng dụng hoặc dừng công việc đang chạy.
4. Dùng `codex` để nhận provider HAC và reasoning `high` trực tiếp từ `config.toml`, không cần alias hoặc `--profile`. `codex --profile hac` vẫn dùng được; profile mặc định cũng là `high`. Thiết lập lại bằng bản HAC mới để cập nhật file đang dùng.

HAC đọc `CODEX_HOME` từ môi trường của chính tiến trình HAC nếu đã đặt (phải là đường dẫn tuyệt đối); mặc định là `~/.codex`, tương ứng `%USERPROFILE%\.codex` trên Windows. Nếu CLI/Desktop/HAC được mở với các giá trị `CODEX_HOME` khác nhau, hãy thống nhất chúng trước. HAC không cài Codex và không thay đổi `auth.json`.

Hai file tạo/cập nhật là `config.toml` và `hac.config.toml`. Provider ID `hac`, tên hiển thị `HAC`, key qua `HAC_API_KEY`, model `gpt-6-astra`, endpoint từ `HAC_BASE_URL` khi build, giao thức Responses, không dùng WebSocket/OpenAI auth. Giữ nguyên model do người dùng chỉ định; khả năng hỗ trợ model phụ thuộc nhà cung cấp, chưa được gọi API kiểm chứng.

HAC cập nhật các root setting yêu cầu, thay toàn bộ bảng `[model_providers.hac]` để tránh thông tin auth cũ xung đột, bật hai tùy chọn memories và bỏ root `profile` cũ để nó không lấn át cấu hình HAC. Các bảng khác, provider khác và nhận xét TOML được giữ lại. Profile HAC cập nhật model/provider/reasoning, giữ thiết lập không liên quan.

## Key và môi trường

- **macOS:** lưu Keychain với service `com.hac.codexmanager`, account `HAC_API_KEY`; gọi `launchctl setenv` cho phiên GUI hiện tại; tạo `~/Library/LaunchAgents/com.hac.codexmanager.environment.plist` và `~/Library/Application Support/HAC/environment.sh` để đọc lại key khi đăng nhập. Script/plist không chứa key. Lệnh `launchctl setenv` cần key làm đối số; đối số có thể hiện trong danh sách tiến trình trong thời gian lệnh chạy. Biến môi trường cũng không có tính bảo mật như Keychain.
- **Shell macOS:** thêm đoạn được đánh dấu HAC vào `.zshenv`, `.bashrc`, login file bash đang dùng và `fish/conf.d/hac.fish`. Các đoạn này đọc Keychain khi shell khởi tạo. Tôn trọng `ZDOTDIR`/`XDG_CONFIG_HOME` nếu HAC nhận được chúng; thư mục tùy chỉnh chỉ đặt bên trong shell mà không được truyền cho HAC không thể tự phát hiện. Keychain bị khóa hoặc từ chối quyền có thể làm bước nạp thất bại; mở khóa và bấm kiểm tra/thiết lập lại. LaunchAgent không đảm bảo thứ tự với các login app: nếu Codex tự mở quá sớm, mở lại sau khi đăng nhập.
- **Windows:** lưu trong Credential Manager, đồng thời ghi chuỗi `HAC_API_KEY` vào `HKCU\Environment` và phát `WM_SETTINGCHANGE`. Bản trong Registry là plaintext theo thiết kế đã chọn, để Codex mở độc lập nhận được key. Không ghi HKLM, không cần Administrator. Tiến trình đã chạy giữ môi trường cũ; nếu Explorer/terminal chưa nhận thay đổi, đăng xuất rồi đăng nhập.

Ứng dụng không ghi key mới vào TOML, script, plist, desktop entry hoặc log; không gửi key đến API. Key được xóa khỏi ô nhập ngay khi bắt đầu lưu. Rust hạn chế thời gian giữ key bằng `Zeroizing`; điều này không đảm bảo xóa mọi bản sao trong IPC/webview/hệ điều hành. Các bản sao lưu có thể chứa dữ liệu nhạy cảm đã có từ trước trong file của người dùng.

## Tính toàn vẹn và khôi phục

Validate tất cả TOML và shell block trước khi thay đổi kho khóa. File bị lỗi hoặc symlink được từ chối. File thay đổi có bản sao lưu `.hac-backup-*` ngay cạnh file gốc, ghi bằng temp file + atomic replace; file mới và backup đặt quyền `0600` trên Unix. Trên Windows quyền được kế thừa từ thư mục người dùng. Khóa file `.hac-setup.lock` ngăn hai HAC cùng áp dụng. So sánh lại nội dung trước khi ghi để không đè một sửa đổi đồng thời.

Nếu một bước thất bại, HAC thử trả file, key và biến môi trường về trạng thái trước đó; lỗi khôi phục được báo rõ. Không đảm bảo transaction nguyên tử qua toàn bộ filesystem/Keychain/Registry khi mất điện hoặc kill tiến trình. Nếu bị gián đoạn, kiểm tra các `.hac-backup-*`, khôi phục file cần thiết và chạy HAC lại. Không xóa backup tự động.

Nút **Kiểm tra cấu hình** chỉ đọc: kiểm tra đúng provider/endpoint/model, key có trong kho khóa, giá trị môi trường hệ điều hành khớp key và cơ chế nạp lại đã có. Không chứng minh tiến trình Codex đang chạy đã nhận key hoặc thực sự gửi traffic tới HAC. Không gửi API request, không phát sinh phí inference. Cần kiểm tra riêng CLI và Desktop sau khi mở lại; không chỉ dựa vào tên profile trong UI.

## Kiểm thử

```sh
npm test
npm run build
cargo test --manifest-path src-tauri/Cargo.toml --no-default-features
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
```

Trên Linux có GNOME Keyring, chạy `bash scripts/test-linux-keyring.sh` để kiểm tra lưu/đọc/đổi/xóa key bằng keyring và secret-tool trong D-Bus riêng với dữ liệu XDG tạm ở `/tmp/hac-keyring.*`. Script không dùng kho khóa đang đăng nhập; không chạy trực tiếp test `--ignored` ngoài phiên cô lập.

Core test dùng thư mục tạm và backend OS giả, không đụng kho khóa/môi trường thật. Có kiểm tra bảo toàn cấu hình, backup, key rotation, rollback từng bước, file lỗi, symlink, sửa đồng thời, IPC camelCase, key không xuất hiện trong kết quả, browser preview, khóa nút khi đang lưu và lỗi UI.

Trước khi phát hành, kiểm thử thủ công trên **macOS, Windows và Linux** bằng tài khoản thử nghiệm: lần đầu, đổi key, từ chối Keychain, file read-only, TOML lỗi, mở lại terminal/Desktop, đăng xuất/đăng nhập và xác nhận endpoint thực tế. Kiểm tra shell zsh/bash/fish và đăng nhập lại trên macOS/Linux; PowerShell/cmd trên Windows. Không in giá trị key trong log hoặc báo cáo kiểm thử.

Tham chiếu cấu hình: [OpenAI Codex Configuration Reference](https://developers.openai.com/codex/config-reference/) (provider ở user-level; profile file nằm cạnh config chính). HAC là công cụ độc lập, không phải sản phẩm chính thức của OpenAI.

## Base URL và GitHub Actions

Source và lịch sử Git công khai không chứa domain thực. Không dùng tiền tố `VITE_` nên URL không bị đưa vào JavaScript frontend. URL vẫn được nhúng trong binary Rust và ghi vào config Codex; người có ứng dụng hoặc quyền đọc cấu hình/lưu lượng mạng vẫn có thể tìm thấy domain. `.env` không phải cơ chế giấu endpoint khỏi người dùng cuối.

Đặt GitHub Actions secret `HAC_BASE_URL` trong repository để đóng gói portable với endpoint thực. CI kiểm thử với URL mẫu; bước đóng gói trên push/workflow_dispatch dùng secret, không chạy trên pull request. Nếu chưa có secret, workflow chỉ kiểm thử và bỏ qua đóng gói trên branch; build tag `v*` sẽ báo lỗi. Không tải artifact CI lên nơi công khai nếu không muốn domain có thể bị trích xuất từ binary.

## Phát hành bằng tag

Sau khi commit và push workflow này, cập nhật phiên bản trong `package.json`, `package-lock.json`, `src-tauri/Cargo.toml`, `src-tauri/Cargo.lock` và `src-tauri/tauri.conf.json` cho khớp bản phát hành, rồi commit và push. Ví dụ phát hành `v0.1.1`:

```sh
git tag v0.1.1
git push origin v0.1.1
```

Workflow chờ cả bốn job macOS arm64/x64, Windows x64 và Linux x64 thành công, rồi tạo GitHub Release cùng tag, sinh release notes và đính kèm ba ZIP portable và một TAR.GZ AppImage. Tải ZIP trực tiếp từ Release chỉ cần giải nén một lần. Chạy lại workflow của tag sẽ cập nhật các asset cùng tên. Push branch, pull request và `workflow_dispatch` không tự tạo Release hoặc tự tăng phiên bản/tag.

## Reset cấu hình và dừng ứng dụng

- **Reset cấu hình Codex:** xác nhận rồi sao lưu và xóa đúng `config.toml`, `hac.config.toml` tại CODEX_HOME đang hiển thị, kể cả TOML lỗi hoặc cấu hình không do HAC tạo. Không xóa auth, hội thoại, credential, shell hoặc biến môi trường. Backup `*.hac-reset-backup-*` được giữ cạnh file gốc (quyền Unix 0600); xem đường dẫn trong Chi tiết. Nhật ký HAC được sao lưu/cập nhật để thiết lập lại hoặc gỡ vẫn hoạt động. Gỡ HAC sau đó vẫn khôi phục baseline trước HAC; không tự xóa backup reset. Khi cần phục hồi thủ công, đóng Codex rồi chép backup tương ứng về tên file gốc; nếu khôi phục cả trạng thái trước reset thì khôi phục cả nhật ký từ backup cùng lần.
- **Dừng ChatGPT & Codex:** xác nhận trước khi buộc dừng ứng dụng Desktop trong tài khoản hiện tại và các tiến trình con nằm trong thư mục ứng dụng. Tác vụ đang chạy bị ngắt, nội dung chưa lưu có thể mất. Không tự mở lại ứng dụng. Không nhắm tới CLI độc lập, trình duyệt hoặc tiến trình tổ tiên của HAC. Dùng nhận diện đường dẫn bundle macOS, package/installation Windows, và executable cạnh `resources/app.asar` trên Linux; bản cài không chuẩn hoặc thiếu quyền đọc process có thể cần đóng thủ công. Số đếm là số tiến trình dừng được, không phải số cửa sổ.

Đóng ứng dụng trước khi reset, sau đó **Lưu và thiết lập HAC**, rồi tự mở lại ChatGPT/Codex. Thao tác reset có backup và rollback khi gặp lỗi; mất điện/kill HAC giữa chừng có thể cần phục hồi từ backup. Không dùng bản HAC cũ để thao tác trên nhật ký đã reset (trạng thái file đã xóa dùng `after: null`).

Theo [OpenAI Docs — Profiles](https://developers.openai.com/codex/config-advanced/#profiles), Codex 0.134.0+ không còn hỗ trợ root `profile = "hac"`. Đặt giá trị chung ở `config.toml` để CLI dùng tự động; mặc định `high` này cũng áp dụng cho Desktop, trừ khi bị override.

## Gỡ thiết lập HAC

Nút **Gỡ thiết lập HAC** yêu cầu xác nhận trước khi xóa key trong kho khóa, xóa biến môi trường user/launchctl/systemd và gỡ cơ chế tự nạp và khôi phục các file đã thay đổi. Bản mới lưu `.hac-restore.json` trong CODEX_HOME (quyền Unix 0600), chứa nội dung file trước/sau, đường dẫn backup và thư mục đã tạo; không chứa API key mới. Nội dung file cũ có thể đã chứa dữ liệu nhạy cảm, nên bảo vệ nhật ký như backup. Khi đổi key, vẫn giữ snapshot trước lần cài đầu tiên. Gỡ sẽ xóa nhật ký, backup được quản lý và thư mục do HAC tạo nếu còn rỗng. Giữ file khóa `.hac-setup.lock` để tránh race giữa các tiến trình; không tự xóa binary đang chạy hoặc dữ liệu WebView của hệ điều hành. Theme preference được xóa sau khi gỡ thành công.

Nếu file đã được sửa ngoài HAC, thao tác dừng trước khi xóa key; đối chiếu/khôi phục backup thủ công rồi thử lại. Mở ứng dụng với cùng CODEX_HOME/ZDOTDIR/XDG_CONFIG_HOME đã dùng khi thiết lập. Cấu hình từ bản cũ không có nhật ký: chỉ khôi phục backup duy nhất khi nội dung hiện tại khớp; nhiều backup hoặc chỉnh sửa mới sẽ chặn gỡ. Nếu không có backup, chỉ loại mục HAC xác định được; không thể tái tạo các giá trị trước đây (ví dụ memories/profile mặc định), nên không khẳng định khôi phục hoàn toàn. Bản mới yêu cầu gỡ bản cũ trước khi thiết lập lại để không tạo snapshot sai.

Gỡ không xóa Codex, auth.json, hội thoại, dự án hoặc toàn bộ thư mục .codex. Đóng rồi mở lại Codex và terminal để loại key còn trong môi trường của tiến trình cũ. Nếu lỗi trong lúc gỡ, HAC thử khôi phục trạng thái trước thao tác và báo nếu khôi phục chưa đầy đủ.
