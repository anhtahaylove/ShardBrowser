# Cloudflare verification cho namhaitravel.com

Tài liệu vận hành này dùng để xử lý các lần automation hợp lệ bị Cloudflare
challenge khi kiểm tra `namhaitravel.com`. Không ghi thật IP, cookie,
`cf_clearance`, Ray ID, token, secret, hoặc header xác thực vào ticket, log hay
Git.

## Phân biệt loại challenge

| Trường hợp | Dấu hiệu | Cách xử lý đúng |
| --- | --- | --- |
| Cloudflare interstitial challenge | Trình duyệt bị chặn trước khi vào origin; trang challenge xuất hiện thay cho nội dung đích. Khi qua challenge, Cloudflare cấp `cf_clearance`. | Điều chỉnh rule Cloudflare/WAF hoặc Challenge Passage. Không sửa form WordPress nếu page chưa tải được. |
| Turnstile nhúng trong trang | Trang `namhaitravel.com` đã tải; widget Turnstile nằm trong form/login/checkout/comment. Origin phải xác minh token qua Siteverify trước khi nhận hành động nhạy cảm. | Sửa cấu hình Turnstile hoặc flow form. Không tạo WAF bypass rộng cho toàn site. |

Turnstile có thể bật pre-clearance để sau khi xác minh, trình duyệt nhận
`cf_clearance` cho các endpoint nhạy cảm cùng hostname. Cookie clearance gắn với
visitor/device, không copy qua máy khác.

## Khi không có quyền quản trị Cloudflare

Giữ xử lý ở phía ShardX thay vì cố tự click challenge:

1. `safe_open_url` phát hiện challenge và pause visible run tối đa 120 giây.
2. Launcher hiển thị `Verification required`; nút `Bring tab to front` kích
   hoạt đúng CDP page target qua `/json/activate/<target-id>`.
3. Người dùng hoàn tất checkbox trong browser profile đang chạy.
4. MCP nhận biết challenge đã clear và tiếp tục trả kết quả cho automation.
5. Giữ nguyên persistent profile để browser tự tái sử dụng `cf_clearance`.

Headless run không tự chờ vì không có cửa sổ cho người dùng xác minh. Có thể đặt
`verification_timeout_ms: 0` để visible run cũng chỉ report trạng thái mà không
pause. Không clear cookie, đổi proxy hoặc thay fingerprint giữa lúc challenge.

## Thiết kế WAF Skip rule hẹp

Chỉ dùng Skip rule khi automation đến từ **một IP tin cậy cố định** và chỉ cần
vào **đúng hostname + đúng path**. Không dùng `Allow` hoặc bypass rộng.

Mẫu điều kiện, thay placeholder trong Cloudflare Dashboard:

```text
(ip.src eq <TRUSTED_AUTOMATION_IP>) and
(http.host eq "namhaitravel.com") and
(http.request.uri.path eq "/exact/automation/path")
```

Với đúng trang trong ảnh hiện tại, điểm bắt đầu hẹp có thể là:

```text
(ip.src eq <TRUSTED_AUTOMATION_IP>) and
(http.host eq "namhaitravel.com") and
(http.request.method in {"GET" "HEAD"}) and
(http.request.uri.path eq "/namhai-rehearsal/wp-admin/")
```

Chỉ Skip feature được Security Events xác định là nguồn challenge. Không Skip
toàn bộ managed WAF rules, rate limiting hoặc các custom rule còn lại. Nếu
WordPress redirect sang một path khác và path đó cũng bị false positive, thêm
đúng path đó sau khi xem Security Events; không đổi thành wildcard/prefix cho
cả thư mục admin và không Skip request `POST` đăng nhập.

Quy tắc vận hành:

- Match chính xác `http.host`; không dùng wildcard hostname.
- Match chính xác `http.request.uri.path`; tránh `contains`, `starts_with`, hoặc
  regex rộng nếu không bắt buộc.
- Không tạo bypass cho toàn `/wp-admin`, `/wp-login.php`, `/xmlrpc.php`, `/wp-json/`,
  hoặc toàn domain.
- Skip đúng sản phẩm đang gây false positive, ví dụ WAF Managed Rules, Browser
  Integrity Check, Security Level, hoặc Super Bot Fight Mode nếu plan hỗ trợ.
- Giữ rate limiting, logging, origin auth, và WordPress auth hoạt động bình
  thường. Skip không thay thế xác thực ứng dụng.
- Đặt tên rule rõ ràng, ví dụ: `automation-ip exact path skip - namhaitravel`.
- Thêm mô tả ticket nội bộ, người tạo, ngày hết hạn dự kiến, và điều kiện rollback.

## Bot Fight Mode Free

Bot Fight Mode trên Free plan không chạy qua Ruleset Engine, nên WAF Custom Rule
với action Skip/Bypass/Allow không bỏ qua được. Nếu Bot Fight Mode Free challenge
traffic automation hợp lệ, chọn một trong hai hướng:

1. Tắt Bot Fight Mode cho zone nếu rủi ro chấp nhận được.
2. Nâng lên Super Bot Fight Mode/Bot Management rồi tạo Skip rule hẹp như trên.

Không mở rộng `/wp-admin` chỉ để né Bot Fight Mode Free; rule đó không giải quyết
đúng nguyên nhân và làm tăng rủi ro admin.

## Phương án tốt hơn cho automation chuyên dụng

Nếu automation là job nội bộ hoặc tích hợp server-to-server, tạo endpoint riêng
thay vì dùng trang admin công khai:

- hostname riêng: `automation.namhaitravel.com`, hoặc
- path riêng: `/automation/<task>`.

Bảo vệ endpoint bằng Cloudflare Access Service Token. Client automation gửi hai
header Access bằng giá trị secret lưu trong secret manager, không hard-code trong
repo hoặc log:

```text
CF-Access-Client-Id: <ACCESS_CLIENT_ID>
CF-Access-Client-Secret: <ACCESS_CLIENT_SECRET>
```

Ở origin, xác minh Access application token/JWT khi có thể để chống đường vòng
không đi qua Cloudflare.

## Turnstile cho staging và production

- Chỉ dùng Turnstile test sitekey/secret trên staging hoặc test suite.
- Không dùng test key trên production `namhaitravel.com`.
- Production secret key sẽ reject dummy token; đây là hành vi đúng.
- Với production, origin phải gọi Siteverify cho mọi token Turnstile trước khi
  chấp nhận login, form, comment, booking, hoặc hành động nhạy cảm.

## Pre-clearance và Challenge Passage

- Pre-clearance phù hợp khi cần một lượt xác minh ban đầu rồi cho phép trình
  duyệt gọi API/path nhạy cảm bằng `cf_clearance`.
- Hostname của Turnstile widget phải khớp zone Cloudflare; sai hostname có thể
  làm clearance không hợp lệ và gây challenge lặp lại.
- Challenge Passage điều khiển thời hạn `cf_clearance`; mặc định của Cloudflare
  là 30 phút và khuyến nghị 15-45 phút. Không đặt quá dài chỉ để tiện automation.
- CORS preflight `OPTIONS` không gửi cookie, nên không dựa vào `cf_clearance` cho
  endpoint cần preflight nếu chưa thiết kế CORS rõ ràng.

## Checklist triển khai

Trước khi bật rule:

- [ ] Xác định challenge là interstitial hay Turnstile nhúng.
- [ ] Xác định đúng Cloudflare feature tạo challenge bằng Security Events.
- [ ] Ghi lại rule hiện tại bằng mô tả không chứa IP thật, Ray ID, cookie, token,
      hoặc secret.
- [ ] Xác nhận automation IP là cố định và thuộc quyền kiểm soát của đội vận hành.
- [ ] Rule chỉ match `namhaitravel.com` và một path chính xác.
- [ ] Không bypass rộng `/wp-admin`, `/wp-login.php`, `/xmlrpc.php`, `/wp-json/`,
      hoặc toàn domain.
- [ ] Nếu là Bot Fight Mode Free, không kỳ vọng Skip rule có hiệu lực.

Sau khi bật rule:

- [ ] Chạy automation từ IP tin cậy và xác nhận đi tới đúng endpoint.
- [ ] Chạy từ IP không tin cậy và xác nhận vẫn bị rule bảo vệ/challenge như cũ.
- [ ] Kiểm tra Cloudflare Security Events chỉ còn match rule hẹp mong muốn.
- [ ] Kiểm tra WordPress/origin vẫn yêu cầu auth và ghi audit log bình thường.
- [ ] Lưu ảnh chụp cấu hình đã che toàn bộ IP, token, cookie, Ray ID, và secret.

Rollback:

- [ ] Disable rule hẹp trước; không xóa ngay để còn so sánh.
- [ ] Xác nhận automation lại bị challenge hoặc bị chặn theo cấu hình cũ.
- [ ] Nếu có sự cố production, tắt rule và khôi phục Bot/WAF setting trước đó.
- [ ] Sau khi ổn định, xóa rule tạm và ghi lại lý do rollback trong ticket nội bộ.

## Nguồn Cloudflare chính thức

- WAF Skip custom rule: <https://developers.cloudflare.com/waf/custom-rules/skip/>
- Bot Fight Mode limitations: <https://developers.cloudflare.com/bots/get-started/bot-fight-mode/>
- Cloudflare challenge clearance: <https://developers.cloudflare.com/cloudflare-challenges/concepts/clearance/>
- Interstitial Challenge Pages: <https://developers.cloudflare.com/cloudflare-challenges/challenge-types/challenge-pages/>
- Challenge Passage: <https://developers.cloudflare.com/cloudflare-challenges/challenge-types/challenge-pages/challenge-passage/>
- Turnstile challenge type: <https://developers.cloudflare.com/cloudflare-challenges/challenge-types/turnstile/>
- Turnstile pre-clearance: <https://developers.cloudflare.com/turnstile/additional-configuration/hostname-management/pre-clearance/>
- Turnstile testing keys: <https://developers.cloudflare.com/turnstile/troubleshooting/testing/>
- Cloudflare Access service tokens: <https://developers.cloudflare.com/cloudflare-one/access-controls/service-credentials/service-tokens/>
- Access application token validation: <https://developers.cloudflare.com/cloudflare-one/access-controls/applications/http-apps/authorization-cookie/application-token/>
