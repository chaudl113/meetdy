# Kế hoạch triển khai: Prompt Templates cho Post-Processing Settings

**Ngày tạo**: 2026-01-11
**Ước lượng công việc**: 18 điểm nhiệm vụ (~26 giờ)
**Agent IDs**:

- UI/UX Design: abd539b
- Planning: ae89d09

---

## 1. Tổng quan tính năng

### 1.1 Mục tiêu

Cung cấp 7 built-in prompt templates để người dùng nhanh chóng sử dụng các prompt phổ biến mà không cần viết từ đầu. Người dùng có thể:

- Xem preview template trước khi sử dụng
- Áp dụng template trực tiếp (không lưu vào custom prompts)
- Tùy chỉnh template và lưu thành custom prompt riêng

### 1.2 7 Built-in Templates

| ID                | Tên                     | Mục đích                             | User persona           |
| ----------------- | ----------------------- | ------------------------------------ | ---------------------- |
| `meeting-summary` | Meeting Summary         | Tóm tắt cuộc họp với key points      | Business professionals |
| `action-items`    | Action Items            | Trích xuất tasks và deadlines        | Project managers       |
| `key-points`      | Key Points              | Liệt kê ý chính dạng bullet          | Note-takers            |
| `translate-vi`    | Translate to Vietnamese | Dịch transcript sang tiếng Việt      | Vietnamese speakers    |
| `grammar-fix`     | Grammar & Clarity       | Sửa ngữ pháp, làm rõ câu văn         | Writers, editors       |
| `email-draft`     | Email Draft             | Chuyển đổi thành email chuyên nghiệp | Business communication |
| `technical-notes` | Technical Notes         | Format thành technical documentation | Developers, engineers  |

### 1.3 Phạm vi

**Bao gồm**:

- 7 built-in templates (constants file)
- UI component: Template Selector với dropdown categorized
- Preview/Edit mode trong PromptPreviewEditor
- "Use Template" workflow (tạm thời áp dụng, không lưu)
- "Save as Custom" workflow (lưu vào custom prompts)
- Đa ngôn ngữ (i18n) cho tất cả UI strings
- Keyboard navigation và accessibility support

**Không bao gồm**:

- User-defined templates (chỉ hỗ trợ custom prompts)
- Template versioning hoặc cloud sync
- AI-powered template suggestions
- Template marketplace

---

## 2. WBS - Phân tách công việc

### Phase 1: Core Data Layer (3 điểm, ~3h)

#### Task 1.1: Tạo file constants cho built-in templates (2 điểm)

**File**: `src/constants/promptTemplates.ts` (NEW)

**Mục tiêu**: Định nghĩa 7 built-in templates với metadata đầy đủ

**Các bước thực hiện**:

1. Tạo file mới `src/constants/promptTemplates.ts`
2. Định nghĩa interface:
   ```typescript
   interface PromptTemplate {
     id: string;
     name: string; // i18n key
     description: string; // i18n key
     category: TemplateCategory;
     prompt: string;
     variables: string[]; // e.g., ["${output}"]
   }
   ```
3. Định nghĩa 7 templates với đầy đủ prompt text
4. Export constants

**Acceptance Criteria**:

- [ ] File tồn tại và compile không lỗi
- [ ] 7 templates có đủ fields (id, name, description, prompt)
- [ ] Mỗi template prompt chứa `${output}` placeholder
- [ ] TypeScript types đầy đủ

**Dependencies**: None

---

#### Task 1.2: Thêm helper functions cho template management (1 điểm)

**File**: `src/constants/promptTemplates.ts`

**Đầu ra**:

- `getTemplateById(id: string): PromptTemplate | undefined`
- `getTemplatesByCategory(category: TemplateCategory): PromptTemplate[]`
- `convertTemplateToLLMPrompt(template: PromptTemplate): Omit<LLMPrompt, 'id'>`

**Acceptance Criteria**:

- [ ] Functions hoạt động đúng với mock data
- [ ] Type-safe với TypeScript strict mode

**Dependencies**: Task 1.1

---

### Phase 2: UI Components (8 điểm, ~11h)

#### Task 2.1: Tạo TemplateDropdown component (3 điểm)

**File**: `src/components/settings/post-processing/TemplateDropdown.tsx` (NEW)

**Mục tiêu**: Dropdown hiển thị danh sách templates theo categories

**UI Spec**:

- Hiển thị grouped list (categories: "Quick Templates", "My Custom Prompts")
- Mỗi item hiển thị: icon + name + description (truncated)
- Keyboard navigation: Arrow keys, Enter, Escape
- Accessibility: ARIA roles, labels

**Acceptance Criteria**:

- [ ] Dropdown hiển thị đúng 7 templates grouped theo category
- [ ] Keyboard nav hoạt động (Arrow Up/Down, Enter, Escape)
- [ ] Click outside đóng dropdown
- [ ] Screen reader đọc được tên và description
- [ ] Tailwind classes tuân theo design system hiện tại

**Dependencies**: Task 1.1

---

#### Task 2.2: Tạo TemplatePreview component (2 điểm)

**File**: `src/components/settings/post-processing/TemplatePreview.tsx` (NEW)

**Mục tiêu**: Preview template với mode "read-only" hoặc "editable"

**UI Spec**:

- Read-only mode: Hiển thị template name + prompt (syntax-highlighted `${output}`)
- Editable mode: Name input + Textarea
- Action buttons: "Use Template", "Save as Custom", "Cancel"

**Acceptance Criteria**:

- [ ] Preview mode hiển thị đúng template info
- [ ] `${output}` được highlight (màu khác hoặc `<code>` tag)
- [ ] Edit mode cho phép sửa name và prompt
- [ ] Buttons trigger đúng callbacks

**Dependencies**: Task 1.1

---

#### Task 2.3: Tạo TemplateSelector container component (3 điểm)

**File**: `src/components/settings/post-processing/TemplateSelector.tsx` (NEW)

**Mục tiêu**: Orchestrate dropdown + preview + workflows

**Workflows**:

1. **Use Template**: Call `onApplyTemplate(template.prompt)` → Update parent state
2. **Save as Custom**: Call Tauri command `addPostProcessPrompt(name, prompt)`

**Acceptance Criteria**:

- [ ] "Use Template" button mở dropdown
- [ ] Chọn template → preview hiển thị
- [ ] "Use Template" workflow hoạt động
- [ ] "Save as Custom" workflow lưu vào backend
- [ ] UI transitions mượt mà

**Dependencies**: Task 2.1, Task 2.2

---

### Phase 3: Integration (4 điểm, ~6h)

#### Task 3.1: Modify PostProcessingSettingsPrompts component (3 điểm)

**File**: `src/components/settings/post-processing/PostProcessingSettings.tsx`

**Mục tiêu**: Tích hợp TemplateSelector vào existing UI

**Vị trí**: Thêm TemplateSelector **trên** dropdown "Select a prompt"

**State Changes**:

- Add `temporaryPrompt: string | null` (for "Use Template" mode)
- Modify logic: If `temporaryPrompt` is set → display it in preview (read-only)

**Acceptance Criteria**:

- [ ] TemplateSelector hiển thị phía trên custom prompt dropdown
- [ ] Apply template → Textarea hiển thị prompt (disabled)
- [ ] "Clear Template" button hoạt động
- [ ] Create new prompt từ template hoạt động
- [ ] Không conflict với existing workflows

**Dependencies**: Task 2.3

---

#### Task 3.2: End-to-end workflow testing (1 điểm)

**Test Cases**:

1. **Use Template workflow**: Apply template trực tiếp, không lưu
2. **Save as Custom workflow**: Lưu template vào custom prompts
3. **Cancel workflow**: Hủy bỏ thay đổi
4. **Keyboard navigation**: Tab, Arrow keys, Enter

**Acceptance Criteria**:

- [ ] Tất cả 4 test cases pass
- [ ] Không có console errors
- [ ] UI responsive và mượt mà

**Dependencies**: Task 3.1

---

### Phase 4: i18n & Accessibility (2 điểm, ~4h)

#### Task 4.1: Thêm i18n keys cho tất cả UI strings (1 điểm)

**File**: `src/i18n/locales/en/translation.json`

**Keys cần thêm** (nested under `settings.postProcessing.prompts`):

- `useTemplate`
- `templates.meetingSummary.{name, description}`
- `templates.actionItems.{name, description}`
- ... (7 templates)
- `templatePreview.{useThisTemplate, saveAsCustom, editTemplate}`

**Acceptance Criteria**:

- [ ] Tất cả UI strings dùng i18n keys
- [ ] ESLint pass (no hardcoded strings)

**Dependencies**: Task 2.1, Task 2.2, Task 2.3

---

#### Task 4.2: Accessibility audit và improvements (1 điểm)

**Checklist**:

- [ ] TemplateDropdown: ARIA roles (`role="menu"`, `role="menuitem"`)
- [ ] Keyboard navigation: Tab order hợp lý
- [ ] Focus indicators: Visible focus ring
- [ ] Screen reader: Test với macOS VoiceOver
- [ ] Color contrast: Min 4.5:1

**Tools**:

- Chrome DevTools Lighthouse
- macOS VoiceOver (Cmd+F5)

**Acceptance Criteria**:

- [ ] Lighthouse Accessibility score ≥ 90
- [ ] VoiceOver đọc được tất cả elements
- [ ] Keyboard-only navigation hoạt động

**Dependencies**: Task 2.1, Task 2.2, Task 3.1

---

### Phase 5: Testing & Polish (1 điểm, ~2h)

#### Task 5.1: Edge cases và error handling (1 điểm)

**Test Scenarios**:

1. **Empty state**: Không có custom prompts
2. **Concurrent edits**: User đang edit custom prompt → Click "Use Template"
3. **Backend failures**: Tauri command fails → Show error toast
4. **Long prompts**: Template prompt > 1000 chars
5. **Special characters**: Emoji, unicode

**Improvements**:

- Add error boundaries
- Add loading states
- Add success toast

**Acceptance Criteria**:

- [ ] Tất cả edge cases handled
- [ ] Error messages user-friendly
- [ ] No crashes

**Dependencies**: Task 3.2

---

## 3. Implementation Sequence

### Sprint 1: Core Data + Basic UI (Day 1-2, 5 điểm)

1. Task 1.1: Template constants (2h)
2. Task 1.2: Helper functions (1h)
3. Task 2.1: TemplateDropdown (4h)

### Sprint 2: Preview + Integration (Day 3-4, 8 điểm)

4. Task 2.2: TemplatePreview (3h)
5. Task 2.3: TemplateSelector (4h)
6. Task 3.1: Integration (4h)

### Sprint 3: Polish + Testing (Day 5, 5 điểm)

7. Task 4.1: i18n (2h)
8. Task 3.2: E2E testing (2h)
9. Task 4.2: Accessibility (2h)
10. Task 5.1: Edge cases (2h)

---

## 4. Dependency Graph

```
T1.1 (Templates) → T1.2 (Helpers)
                 ↓
T2.1 (Dropdown) ─┬→ T2.3 (Selector) → T3.1 (Integration) → T3.2 (E2E) → T5.1 (Polish)
T2.2 (Preview) ──┘                                          ↓
                                                      T4.1 (i18n), T4.2 (A11y)
```

**Critical Path**: T1.1 → T2.3 → T3.1 → T3.2 → T5.1 (11 điểm)

---

## 5. Risk Assessment

| Risk                             | Likelihood | Impact | Mitigation                        |
| -------------------------------- | ---------- | ------ | --------------------------------- |
| State management conflict        | High       | Medium | Refactor early, add state diagram |
| Template prompts không effective | Medium     | High   | Test với GPT-4 trước, dễ update   |
| Keyboard nav conflict            | Medium     | Low    | Event propagation cẩn thận        |
| Accessibility issues             | Low        | Medium | Lighthouse audit mỗi component    |

---

## 6. File Checklist

### Files to Create:

- [ ] `src/constants/promptTemplates.ts`
- [ ] `src/components/settings/post-processing/TemplateDropdown.tsx`
- [ ] `src/components/settings/post-processing/TemplatePreview.tsx`
- [ ] `src/components/settings/post-processing/TemplateSelector.tsx`

### Files to Modify:

- [ ] `src/components/settings/post-processing/PostProcessingSettings.tsx`
- [ ] `src/i18n/locales/en/translation.json`

---

## 7. Overall Acceptance Criteria

**Feature complete khi**:

- [ ] 7 built-in templates hiển thị trong dropdown
- [ ] "Use Template" workflow hoạt động
- [ ] "Save as Custom" workflow lưu vào backend
- [ ] i18n hoàn chỉnh
- [ ] Keyboard navigation hoạt động
- [ ] Lighthouse Accessibility score ≥ 90
- [ ] Không có TypeScript/ESLint errors
- [ ] Manual testing pass
- [ ] Backward compatibility: Existing custom prompts hoạt động

---

## 8. Success Metrics

**Objective metrics**:

- Lighthouse score: ≥ 90
- TypeScript errors: 0
- ESLint warnings: 0
- Build time increase: < 10%

**Subjective metrics** (User feedback):

- Template prompts hữu ích và chính xác
- UI intuitive và dễ sử dụng
- Workflows mượt mà

---

**Tổng kết**: 10 tasks, 18 điểm, ~26 giờ làm việc
