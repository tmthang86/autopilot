# Kịch bản kiểm tra thủ công: 6 mục "needs-human" còn lại

Đây là runbook cho các mục trong [`docs/product/open-items.md`](../product/open-items.md) mà máy
không tự xác nhận được — cần người quan sát thật. Không mục nào ở đây có thể chạy qua autopilot's
own automation; đó chính là lý do chúng còn nằm ở "Unverified guarantees" / "Open questions".

Thứ tự dưới đây có phụ thuộc: mục 1 (preflight) phải chạy trước mục 2 (3-role run) vì mục 2 cần
tier binding mà preflight tạo ra.

Sau mỗi mục: cập nhật dòng tương ứng trong `docs/product/open-items.md` — xóa khỏi bảng nếu đã xác
nhận xong, hoặc sửa lại nếu kết quả khác dự kiến (đừng chỉ tích ✅ mà không sửa nội dung nếu thực tế
khác).

---

## Chuẩn bị chung

```sh
cd /Volumes/KINGSTON-DRIVE/Projects/autopilot/runner
cargo build --release
./target/release/autopilot --help   # xác nhận binary chạy được, in đúng usage
```

Binary nằm tại `runner/target/release/autopilot`. Toàn bộ lệnh dưới đây dùng đường dẫn tuyệt đối
tới binary này (chưa deploy qua `deploy.sh`, đó là một phần của việc cutover ở mục 2).

Cần một **project thật** để test — không phải chính repo `autopilot`. Nếu chưa có sẵn project nào
để thử, tạo một repo GitHub trống mới (private là được) và clone về máy; các bước dưới giả định biến
`$PROJ` trỏ tới thư mục đó và bạn đã `gh auth login`.

```sh
export PROJ=/path/to/some/test/project
```

---

## 1. `autopilot preflight` trên máy thật, hai lần

**Mục tiêu:** xác nhận preflight báo đúng harness nào máy này thực sự dùng được, sửa những gì nó
báo lỗi, rồi chạy lại phải sạch.

```sh
cd "$PROJ"
/Volumes/KINGSTON-DRIVE/Projects/autopilot/runner/target/release/autopilot install --project "$PROJ"
/Volumes/KINGSTON-DRIVE/Projects/autopilot/runner/target/release/autopilot preflight --project "$PROJ"
```

**Kỳ vọng lần 1:** JSON liệt kê từng harness (`claude`, `pi`, `opencode`) với `available`,
`proven`, `models`, `error`. Theo `docs/reference/observed-behaviour.md` (đo ngày 2026-08-19), máy
này lúc đó có:
- `claude` — sẵn sàng, đã proven
- `pi` — sẵn sàng (deepseek), đã proven
- `opencode` — lỗi vì `~/.config/opencode/config.json` thiếu `mcp.gitnexus.enabled`

Nếu `opencode` vẫn báo lỗi đó, đây là lúc sửa: mở file config, thêm `"enabled": true` (hoặc field
đúng theo thông báo lỗi in ra), rồi chạy lại preflight lần 2.

**Kỳ vọng lần 2:** không harness nào còn báo lỗi cấu hình có thể sửa được ở máy này (một máy không
cài `opencode`/`pi` thì báo "not found" là đúng, không phải lỗi cần sửa). `unresolved` (tier không
resolve được) phải rỗng nếu `.autopilot/tiers.local.json` đã khớp với harness thật sự sẵn sàng.

**Ghi lại:** cập nhật dòng `autopilot preflight against the machine as-is...` trong open-items.md —
xóa nếu sạch, hoặc note lại lỗi nào không sửa được (ví dụ máy không có `opencode` cài đặt thật).

---

## 2. Một run 3-role thật qua harness thật — rồi mới cutover

**Mục tiêu:** issue thật, chạy implement→test→review thật (không stub), verify xanh, issue đóng.
Đây là điều kiện Task 17 của `docs/plans/2026-08-16-rust-runner.md` yêu cầu trước khi xóa shell
runner cũ và chuyển `deploy.sh`/plist sang binary Rust.

**Chuẩn bị:**
1. `$PROJ` phải có `.autopilot/config.json` với `verify` trỏ đúng lệnh test thật của project đó
   (không phải `true` giả).
2. Tạo một issue GitHub thật, nội dung nhỏ, rõ ràng, có dòng `Intent:` trỏ tới một file thật đã tồn
   tại trong repo, và nhãn `tier:<tên tier trong config.json>` cộng nhãn ready (`autopilot` theo
   mặc định).

```sh
gh issue create --repo <owner>/<repo> \
  --title "Test: add a one-line comment to README" \
  --body "Add a comment to README.md explaining what this repo does.

Intent: README.md" \
  --label autopilot --label tier:light
```

**Chạy:**
```sh
/Volumes/KINGSTON-DRIVE/Projects/autopilot/runner/target/release/autopilot run-once --project "$PROJ"
tail -f "$PROJ/.autopilot/logs/$(date -u +%Y-%m-%d).log"
```

**Quan sát trong lúc chạy:**
- Log có dòng cho từng role (implement, test, review) — role nào bắt đầu phải có role nào kết thúc
  tương ứng, không role nào "treo".
- `$PROJ/.autopilot/journal.jsonl` — mỗi `role_start` có `role_end` khớp `wake`/`round`/`lens`.

**Kỳ vọng khi xong:** issue đóng trên GitHub với comment tự động, commit thật xuất hiện trên nhánh
`autopilot/main` của `$PROJ` (không phải `main`), verify chạy xanh trước khi commit.

**Chỉ sau khi mục này pass thật:**
```sh
cd /Volumes/KINGSTON-DRIVE/Projects/autopilot/runner
sh deploy.sh          # chuyển sang binary ổn định — đọc script trước khi chạy
```
rồi cập nhật launchd plist của các project đang chạy để trỏ sang binary thay vì shell runner, và
**chỉ khi đó** mới xóa `runner/lib/*.sh` + `runner/run-once.sh` cũ.

**Ghi lại:** cập nhật dòng "A real three-role run..." trong open-items.md.

---

## 3. Dashboard theo dõi project thật đang chạy

**Mục tiêu:** orphan detection, in-flight roles, cost figures khớp với thực tế — không chỉ fixture.

```sh
cd /Volumes/KINGSTON-DRIVE/Projects/autopilot/dashboard
go build -o /tmp/apdash .
/tmp/apdash --jobs ~/.local/share/autopilot/jobs --addr 127.0.0.1:8787
```

Mở `http://127.0.0.1:8787` trong lúc mục 2 đang chạy (hoặc chạy lại một task khác song song). Nếu
muốn cố tình tạo một orphan để test, `kill -9` tiến trình `autopilot run-once` giữa chừng rồi refresh
dashboard — role đó phải hiện lên là orphan, không biến mất.

**Kỳ vọng:** trang overview hiện đúng project, đúng số role in-flight, đúng chi phí (`reported` vs
`unknown` tách biệt, không cộng gộp), và nếu có orphan thì nó nổi bật lên trên.

**Ghi lại:** cập nhật dòng "The dashboard against a real running project..." trong open-items.md.

---

## 4. Planning-skill end-to-end trên repo thật có remote thật

**Mục tiêu:** chạy `autopilot-planning` → `autopilot-deliver` thật trên một repo throwaway có remote
GitHub thật (không mô phỏng), xác nhận toàn bộ chu trình idea → plan → issue → shard hoạt động.

Chạy trong Claude Code, trỏ vào `$PROJ` (hoặc một repo throwaway mới), gọi skill
`autopilot-planning` với một ý tưởng nhỏ thật, đi hết chu trình tới lúc `autopilot-deliver` tạo
issue thật trên GitHub và (nếu muốn) tiếp tục qua mục 2 để thấy issue đó được autopilot nhặt lên
thật.

**Ghi lại:** cập nhật dòng cuối cùng chưa có trong bảng — mục Task 9 của
`docs/plans/2026-08-16-planning-skill.md` mô tả chính xác bước này; đối chiếu lại theo đó.

---

## 5. Reboot: `stop` phải sống sót qua reboot, job đã `stop` không tự quay lại

**Mục tiêu:** xác nhận ADR-0002 đúng trên thực tế, không chỉ trên giấy.

```sh
sh ~/.local/share/autopilot/runner/ctl.sh start "$PROJ"
sh ~/.local/share/autopilot/runner/ctl.sh status "$PROJ"   # loaded: yes
sh ~/.local/share/autopilot/runner/ctl.sh stop  "$PROJ"
sh ~/.local/share/autopilot/runner/ctl.sh status "$PROJ"   # loaded: no
```
Reboot máy. Sau khi đăng nhập lại:
```sh
sh ~/.local/share/autopilot/runner/ctl.sh status "$PROJ"   # vẫn phải là loaded: no
```

Sau đó test chiều ngược lại — start rồi reboot, job phải tự quay lại (`StartInterval`/`RunAtLoad`
đúng như thiết kế cho phiên đã start):
```sh
sh ~/.local/share/autopilot/runner/ctl.sh start "$PROJ"
# reboot
sh ~/.local/share/autopilot/runner/ctl.sh status "$PROJ"   # loaded: yes
```

**Ghi lại:** cập nhật dòng "A reboot: that stop survives one..." trong open-items.md — mục này đã
nợ từ 2026-08-15, lâu nhất trong danh sách.

---

## 6. `opencode` có tới được `ollama`/`lmstudio` không

**Mục tiêu:** đây là route duy nhất cho local-model tier — cần biết có thật sự dùng được không.

```sh
# sau khi opencode đã sửa xong ở mục 1
opencode models 2>&1 | grep -i "ollama\|lmstudio"
```

Nếu máy có cài `ollama` hoặc `lmstudio` chạy local, thử một lệnh thật qua opencode nhắm vào model
đó và xem có trả lời được không (không chỉ liệt kê model).

**Ghi lại:** cập nhật hai dòng trong "Open questions" của open-items.md — chuyển câu trả lời từ "Open
questions" sang "Unverified guarantees" đã settle, hoặc xóa hẳn nếu đã có câu trả lời chắc chắn.

---

## Sau khi xong cả 6 mục

Đọc lại toàn bộ `docs/product/open-items.md` — nếu bảng "Unverified guarantees" trống, đó là tín
hiệu để cân nhắc đóng dấu dự án là production-ready thật sự, không chỉ "implemented".
