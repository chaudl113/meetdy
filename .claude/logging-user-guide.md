# 📚 Hướng Dẫn Sử Dụng Logging System - Meetdy

## 🎯 Mục Đích

Logging system giúp bạn:

- 🔍 Debug các vấn đề khi meeting không hoạt động đúng
- ⚡ Phân tích performance để tối ưu hóa
- 📊 Theo dõi độ tin cậy của hệ thống
- 🐛 Tìm và fix bugs nhanh chóng

## 📁 Vị Trí Log Files

### macOS

```
~/Library/Logs/com.handy.app/meetdy.log
```

### Cách Mở Nhanh

```bash
# Mở thư mục logs
open ~/Library/Logs/com.handy.app/

# Xem logs realtime
tail -f ~/Library/Logs/com.handy.app/meetdy.log

# Mở bằng Console.app (macOS built-in)
open -a Console ~/Library/Logs/com.handy.app/meetdy.log
```

## 🔍 Cách Đọc Logs

### Log Format

Mỗi log line có format:

```
[TIMESTAMP] [LEVEL] [PREFIX] [SESSION_ID] operation - message
```

**Ví dụ:**

```
[2026-01-11 14:30:15] INFO [MEETING] [abc-123] start_recording - Started
```

### Log Prefixes

| Prefix             | Ý Nghĩa                  |
| ------------------ | ------------------------ |
| `[MEETING]`        | Meeting operations chung |
| `[MEETING_START]`  | Bắt đầu recording        |
| `[MEETING_STOP]`   | Dừng recording           |
| `[MIC_DISCONNECT]` | Mic bị disconnect        |
| `[APP_SHUTDOWN]`   | App đang tắt             |
| `[WAV_FINALIZE]`   | Lưu file audio           |
| `[MEETING_EVENT]`  | Events quan trọng        |
| `[MEETING_METRIC]` | Performance metrics      |

### Log Levels

| Level   | Khi Nào Xuất Hiện         |
| ------- | ------------------------- |
| `ERROR` | Lỗi nghiêm trọng          |
| `WARN`  | Cảnh báo (không critical) |
| `INFO`  | Thông tin quan trọng      |
| `DEBUG` | Chi tiết debug            |

## 🎬 Các Scenarios Thường Gặp

### 1. Meeting Bắt Đầu Thành Công

**Logs bạn sẽ thấy:**

```
[MEETING_START] Creating session with audio source: MicrophoneOnly
[MEETING] [abc-123] start_recording - Started
[MEETING] [abc-123] start_recording - Success (25ms): Session started
[MEETING_EVENT] session=abc-123 event=session_started details=source=MicrophoneOnly
```

**✅ Dấu hiệu tốt:**

- Có message "Success"
- Timing <100ms
- Có session ID rõ ràng

### 2. Meeting Dừng Thành Công

**Logs bạn sẽ thấy:**

```
[MEETING] [abc-123] stop_recording - Started
[MEETING] [abc-123] stop_recording - Timing: wav_finalize = 45ms
[MEETING_METRIC] session=abc-123 metric=recording_duration value=120.5 unit=seconds
[MEETING] [abc-123] stop_recording - Success (62ms): Recording stopped
[MEETING_EVENT] session=abc-123 event=recording_stopped details=duration=120s
```

**✅ Dấu hiệu tốt:**

- `wav_finalize` <100ms
- Có `recording_duration` metric
- Success message với duration chính xác

### 3. Microphone Bị Disconnect

**Logs bạn sẽ thấy:**

```
[MIC_DISCONNECT] Detected: Audio input device disconnected
[MEETING] [abc-123] handle_mic_disconnect - Started
[MEETING] [abc-123] handle_mic_disconnect - Error: Audio input device disconnected
[MEETING] [abc-123] handle_mic_disconnect - State transition: Recording -> Failed
[MEETING_EVENT] session=abc-123 event=mic_disconnected details=error=...
```

**🔍 Cách khắc phục:**

1. Kiểm tra mic có plug in không
2. Check System Preferences → Sound → Input
3. Restart app và thử lại

### 4. App Tắt Khi Đang Recording

**Logs bạn sẽ thấy:**

```
[APP_SHUTDOWN] Handling app shutdown for meeting sessions
[MEETING] [abc-123] handle_app_shutdown - Warning: Interrupting recording
[MEETING] [abc-123] handle_app_shutdown - State transition: Recording -> Interrupted
[MEETING_EVENT] session=abc-123 event=app_shutdown_interrupted details=duration=45s
```

**✅ Điều tốt:**

- Audio đã được save (partial)
- Duration được track
- Session có thể recover

## 🔧 Commands Hữu Ích

### Xem Logs Theo Thời Gian Thực

```bash
tail -f ~/Library/Logs/com.handy.app/meetdy.log
```

### Lọc Chỉ Meeting Operations

```bash
grep "\[MEETING\]" ~/Library/Logs/com.handy.app/meetdy.log
```

### Xem Performance Metrics

```bash
grep "MEETING_METRIC" ~/Library/Logs/com.handy.app/meetdy.log
```

### Đếm Số Meetings Đã Record

```bash
grep -c "session_started" ~/Library/Logs/com.handy.app/meetdy.log
```

### Xem Các Lỗi

```bash
grep -E "ERROR|Failed|failed" ~/Library/Logs/com.handy.app/meetdy.log
```

### Tìm Session Cụ Thể

```bash
# Thay abc-123 bằng session ID của bạn
grep "abc-123" ~/Library/Logs/com.handy.app/meetdy.log
```

### Xem Log Hôm Nay

```bash
grep "$(date +%Y-%m-%d)" ~/Library/Logs/com.handy.app/meetdy.log
```

## 🐛 Troubleshooting

### Log File Không Tồn Tại

**Nguyên nhân:**

- App chưa chạy lần đầu
- Log level setting = Off
- Không có permission ghi file

**Giải pháp:**

1. Chạy app ít nhất 1 lần
2. Check Settings → Debug → Log Level (should be Debug hoặc Info)
3. Check folder permissions: `ls -l ~/Library/Logs/`

### Log File Quá Lớn

**Khi nào xảy ra:**

- File > 5MB sẽ rotate
- Files cũ: `meetdy.log.1`, `meetdy.log.2`, etc.

**Giải pháp:**

```bash
# Xem tất cả log files
ls -lh ~/Library/Logs/com.handy.app/

# Xóa logs cũ (cẩn thận!)
rm ~/Library/Logs/com.handy.app/meetdy.log.*

# Hoặc archive
tar -czf ~/Desktop/meetdy-logs-$(date +%Y%m%d).tar.gz ~/Library/Logs/com.handy.app/
```

### WAV Finalization Chậm

**Dấu hiệu trong logs:**

```
[WAV_FINALIZE] Lock acquired after 50 retries (1200ms), finalizing...
```

**Nếu thấy:**

- Retry count > 10
- Finalization time > 1000ms

**Có nghĩa:**

- Có race condition
- System đang quá tải
- Disk I/O chậm

**Giải pháp:**

1. Close các apps khác
2. Check disk space: `df -h`
3. Restart app
4. Report issue với logs

### State Transition Error

**Logs:**

```
[MEETING_STOP] Rejected: session already processing
```

**Nguyên nhân:**

- Click Stop button quá nhanh (double-click)
- Session đang trong processing state

**Giải pháp:**

- Đợi vài giây trước khi click lại
- Check session status trên UI

## 📊 Hiểu Performance Metrics

### Recording Duration

```
[MEETING_METRIC] session=abc-123 metric=recording_duration value=120.5 unit=seconds
```

- **Ý nghĩa:** Tổng thời gian đã record
- **Tốt:** Giống với thời gian hiển thị trên UI
- **Xấu:** Khác biệt >2s → có bug timing

### WAV Finalize Time

```
[MEETING] [abc-123] stop_recording - Timing: wav_finalize = 45ms
```

- **Tốt:** <100ms
- **Chấp nhận được:** 100-500ms
- **Xấu:** >500ms → cần investigation

### Recorder Start/Stop Time

```
[MEETING] [abc-123] start_recording - Timing: recorder_start = 15ms
[MEETING] [abc-123] stop_recording - Timing: recorder_stop = 8ms
```

- **Tốt:** <50ms
- **Chấp nhận được:** 50-200ms
- **Xấu:** >200ms → audio device issue

## 🎯 Khi Nào Cần Share Logs

### Report Bug

**Cần include:**

1. Session ID (tìm trong logs)
2. Timestamp của issue
3. Error messages
4. 20-30 lines trước và sau error

**Cách export:**

```bash
# Lấy logs của session cụ thể
grep "abc-123" ~/Library/Logs/com.handy.app/meetdy.log > ~/Desktop/bug-report-abc-123.txt

# Hoặc lấy logs hôm nay
grep "$(date +%Y-%m-%d)" ~/Library/Logs/com.handy.app/meetdy.log > ~/Desktop/logs-today.txt
```

### Performance Issue

**Cần include:**

1. All MEETING_METRIC lines
2. WAV_FINALIZE lines với timing
3. System specs (CPU, RAM, disk space)

**Cách export:**

```bash
# Lấy metrics
grep "MEETING_METRIC" ~/Library/Logs/com.handy.app/meetdy.log > ~/Desktop/metrics.txt

# Lấy WAV finalization data
grep "WAV_FINALIZE" ~/Library/Logs/com.handy.app/meetdy.log > ~/Desktop/wav-perf.txt
```

## 📞 Support

Nếu cần help:

1. **Gather logs** theo hướng dẫn trên
2. **Note down:**
   - Thời gian xảy ra issue
   - Steps to reproduce
   - Expected vs actual behavior
3. **Create issue** với logs attached

---

**Happy Debugging!** 🎉

Logging system này được thiết kế để giúp bạn tự debug và tối ưu app. Nếu có câu hỏi hoặc cần thêm features, đừng ngần ngại ask!
