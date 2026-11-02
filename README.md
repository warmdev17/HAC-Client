# HAC — Thiết lập Codex

Ứng dụng Tauri 2 + TypeScript cấu hình Codex Desktop và CLI bằng provider HAC trên **macOS và Windows native**. Linux chỉ dùng để phát triển giao diện/kiểm thử core; không ghi key hoặc áp dụng cấu hình. Không hỗ trợ WSL trong bản này.

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

Installer nằm trong `src-tauri/target/release/bundle/`. Workflow GitHub Actions chỉ xuất ZIP portable (không tạo installer), gồm `HAC-Windows-x64-portable`, `HAC-macOS-arm64-portable` và `HAC-macOS-x64-portable`; không tự phát hành. Windows giải nén rồi mở `hac-codex-manager.exe` (cần WebView2 Runtime). macOS giải nén rồi mở `HAC.app`; ZIP tạo bằng `ditto` để giữ quyền thực thi và symlink. Artifact GitHub là ZIP bọc ngoài ZIP portable, nên có thể cần giải nén hai lần. Portable vẫn lưu cấu hình/key vào tài khoản người dùng khi bấm thiết lập. Chưa cấu hình chứng chỉ ký code/notarization. Muốn phân phối rộng rãi cần thêm chứng chỉ macOS/Windows vào CI. Bản macOS mặc định theo kiến trúc runner/máy build; dùng target phù hợp nếu cần Intel/Apple Silicon riêng.

`npm run dev` mở bản xem trước web tại `http://127.0.0.1:1420`; chức năng ghi key/cấu hình bị khóa vì không có backend Tauri. Giao diện dùng font hệ thống, không tải font hoặc nội dung từ bên ngoài.

## Sử dụng

1. Mở HAC trong tài khoản người dùng thông thường.
2. Nhập API key và bấm **Lưu và thiết lập HAC**. Cửa sổ Keychain của macOS có thể yêu cầu quyền.
3. Xem kết quả và đường dẫn bản sao lưu. Đóng hoàn toàn Codex Desktop và terminal, sau đó mở lại. Không tự đóng ứng dụng hoặc dừng công việc đang chạy.
4. Dùng `codex` với reasoning `medium`, hoặc `codex --profile hac` với reasoning `high`.

HAC đọc `CODEX_HOME` từ môi trường của chính tiến trình HAC nếu đã đặt (phải là đường dẫn tuyệt đối); mặc định là `~/.codex`, tương ứng `%USERPROFILE%\.codex` trên Windows. Nếu CLI/Desktop/HAC được mở với các giá trị `CODEX_HOME` khác nhau, hãy thống nhất chúng trước. HAC không cài Codex và không thay đổi `auth.json`.

Hai file tạo/cập nhật là `config.toml` và `hac.config.toml`. Provider ID `hac`, tên hiển thị `HAC`, key qua `HAC_API_KEY`, model `gpt-6-astra`, endpoint từ `HAC_BASE_URL` khi build, giao thức Responses, không dùng WebSocket/OpenAI auth. Giữ nguyên model do người dùng chỉ định; khả năng hỗ trợ model phụ thuộc nhà cung cấp, chưa được gọi API kiểm chứng.

HAC cập nhật các root setting yêu cầu, thay toàn bộ bảng `[model_providers.hac]` để tránh thông tin auth cũ xung đột, bật hai tùy chọn memories và bỏ root `profile` cũ để nó không lấn át cấu hình HAC. Các bảng khác, provider khác và nhận xét TOML được giữ lại. Profile HAC cập nhật model/provider/reasoning, giữ thiết lập không liên quan.

## Key và môi trường

- **macOS:** lưu Keychain với service `com.hac.codexmanager`, account `HAC_API_KEY`; gọi `launchctl setenv` cho phiên GUI hiện tại; tạo `~/Library/LaunchAgents/com.hac.codexmanager.environment.plist` và `~/Library/Application Support/HAC/environment.sh` để đọc lại key khi đăng nhập. Script/plist không chứa key. Lệnh `launchctl setenv` cần key làm đối số; đối số có thể hiện trong danh sách tiến trình trong thời gian lệnh chạy. Biến môi trường cũng không có tính bảo mật như Keychain.
- **Shell macOS:** thêm đoạn được đánh dấu HAC vào `.zshenv`, `.bashrc`, login file bash đang dùng và `fish/conf.d/hac.fish`. Các đoạn này đọc Keychain khi shell khởi tạo. Tôn trọng `ZDOTDIR`/`XDG_CONFIG_HOME` nếu HAC nhận được chúng; thư mục tùy chỉnh chỉ đặt bên trong shell mà không được truyền cho HAC không thể tự phát hiện. Keychain bị khóa hoặc từ chối quyền có thể làm bước nạp thất bại; mở khóa và bấm kiểm tra/thiết lập lại. LaunchAgent không đảm bảo thứ tự với các login app: nếu Codex tự mở quá sớm, mở lại sau khi đăng nhập.
- **Windows:** lưu trong Credential Manager, đồng thời ghi chuỗi `HAC_API_KEY` vào `HKCU\Environment` và phát `WM_SETTINGCHANGE`. Bản trong Registry là plaintext theo thiết kế đã chọn, để Codex mở độc lập nhận được key. Không ghi HKLM, không cần Administrator. Tiến trình đã chạy giữ môi trường cũ; nếu Explorer/terminal chưa nhận thay đổi, đăng xuất rồi đăng nhập.

Ứng dụng không ghi key mới vào TOML, script, plist hoặc log; không gửi key đến API. Key được xóa khỏi ô nhập ngay khi bắt đầu lưu. Rust hạn chế thời gian giữ key bằng `Zeroizing`; điều này không đảm bảo xóa mọi bản sao trong IPC/webview/hệ điều hành. Các bản sao lưu có thể chứa dữ liệu nhạy cảm đã có từ trước trong file của người dùng.

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

Core test dùng thư mục tạm và backend OS giả, không đụng kho khóa/môi trường thật. Có kiểm tra bảo toàn cấu hình, backup, key rotation, rollback từng bước, file lỗi, symlink, sửa đồng thời, IPC camelCase, key không xuất hiện trong kết quả, browser preview, khóa nút khi đang lưu và lỗi UI.

Trước khi phát hành, kiểm thử thủ công trên **cả macOS và Windows** bằng tài khoản thử nghiệm: lần đầu, đổi key, từ chối Keychain, file read-only, TOML lỗi, mở lại terminal/Desktop, đăng xuất/đăng nhập và xác nhận endpoint thực tế. Kiểm tra shell zsh/bash/fish trên macOS; PowerShell/cmd trên Windows. Không in giá trị key trong log hoặc báo cáo kiểm thử.

Tham chiếu cấu hình: [OpenAI Codex Configuration Reference](https://developers.openai.com/codex/config-reference/) (provider ở user-level; profile file nằm cạnh config chính). HAC là công cụ độc lập, không phải sản phẩm chính thức của OpenAI.

## Base URL và GitHub Actions

Source và lịch sử Git công khai không chứa domain thực. Không dùng tiền tố `VITE_` nên URL không bị đưa vào JavaScript frontend. URL vẫn được nhúng trong binary Rust và ghi vào config Codex; người có ứng dụng hoặc quyền đọc cấu hình/lưu lượng mạng vẫn có thể tìm thấy domain. `.env` không phải cơ chế giấu endpoint khỏi người dùng cuối.

Đặt GitHub Actions secret `HAC_BASE_URL` trong repository để đóng gói ZIP portable với endpoint thực. CI kiểm thử với URL mẫu; bước đóng gói trên push/workflow_dispatch dùng secret, không chạy trên pull request. Nếu chưa có secret, workflow chỉ kiểm thử và bỏ qua đóng gói. Không tải artifact CI lên nơi công khai nếu không muốn domain có thể bị trích xuất từ binary.
