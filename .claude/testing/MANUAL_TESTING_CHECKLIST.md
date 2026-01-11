# Prompt Templates - Manual Testing Checklist

**App Status**: ✅ Running at http://localhost:1420/
**Feature**: Built-in Prompt Templates for Post-Processing
**Date**: 2026-01-11

---

## Pre-Testing Setup

- [ ] **Enable Post-Processing**
  - Navigate to: Settings → Debug
  - Toggle ON: "Post Processing"

- [ ] **Configure API** (if not already done)
  - Navigate to: Settings → Post Process → API
  - Select provider: OpenAI / Custom / Apple Intelligence
  - Enter API key (if required)
  - Select model

---

## 🎯 Quick Test (5 minutes)

**Goal**: Verify basic template workflow works

### Step-by-Step:

1. **Navigate to Prompts Section**
   - [ ] Go to: Settings → Post Process → Prompts
   - [ ] Locate "Use Template" button above custom prompts dropdown

2. **Open Template Dropdown**
   - [ ] Click "Use Template" button
   - [ ] Verify dropdown opens with categorized templates
   - [ ] Verify you see 3 categories: MEETING, LANGUAGE, WRITING

3. **Apply a Template**
   - [ ] Select "Meeting Summary" (or any template)
   - [ ] Verify preview shows template content
   - [ ] Verify `${output}` placeholder is highlighted in yellow/primary color
   - [ ] Click "Use Template" button in preview
   - [ ] Verify template content appears in textarea
   - [ ] Verify blue indicator shows: "Using template: **Meeting Summary**"

4. **Clear Template**
   - [ ] Click "Clear" button
   - [ ] Verify template content is removed
   - [ ] Verify blue indicator disappears

5. **Save as Custom Prompt**
   - [ ] Click "Use Template" again
   - [ ] Select "Grammar & Clarity"
   - [ ] Click "Use Template"
   - [ ] Click "Save as Custom" button
   - [ ] Verify custom prompt is created
   - [ ] Verify it appears in dropdown above
   - [ ] Verify you can edit and update it

---

## 🔍 Detailed Test (15 minutes)

### Test 1: All 7 Templates Present

**Categories to verify**:

**MEETING**
- [ ] ✅ Meeting Summary
- [ ] 📋 Extract Action Items
- [ ] 🔑 Key Points

**LANGUAGE**
- [ ] 🇻🇳 Translate to Vietnamese

**WRITING**
- [ ] ✏️ Grammar & Clarity
- [ ] 📧 Convert to Email
- [ ] 📝 Technical Documentation

### Test 2: Dropdown Interactions

- [ ] Click "Use Template" → Dropdown opens
- [ ] Click outside dropdown → Dropdown closes
- [ ] Click "Use Template" again → Dropdown opens
- [ ] Press Escape key → Dropdown closes
- [ ] Select a template → Dropdown auto-closes

### Test 3: Keyboard Navigation

- [ ] Press Tab to focus "Use Template" button
- [ ] Press Enter → Dropdown opens
- [ ] Press Arrow Down → Next template highlights
- [ ] Press Arrow Up → Previous template highlights
- [ ] Press Enter on highlighted template → Template selected
- [ ] Tab through preview buttons → All focusable
- [ ] Press Escape → Dropdown closes

### Test 4: Edit Before Save

- [ ] Click "Use Template"
- [ ] Select any template
- [ ] In preview mode, click "Edit and save as custom prompt"
- [ ] Verify mode switches to edit mode with editable fields
- [ ] Edit template name (e.g., "My Custom Summary")
- [ ] Edit prompt text
- [ ] Click "Save as Custom"
- [ ] Verify custom prompt saves with edited content
- [ ] Verify original built-in template remains unchanged

### Test 5: Cancel Workflow

- [ ] Click "Use Template"
- [ ] Select a template
- [ ] Click "Cancel" in preview → Verify preview closes
- [ ] Click "Use Template" again
- [ ] Select template → Click "Use Template"
- [ ] Verify template applies
- [ ] Click "Cancel" (next to "Save as Custom")
- [ ] Verify applied template clears

### Test 6: Integration with Custom Prompts

- [ ] Create a new custom prompt using "Create New Prompt"
- [ ] Verify it appears in dropdown
- [ ] Select it → Verify editable
- [ ] Apply a template → Verify custom prompt deselects
- [ ] Clear template → Select custom prompt again
- [ ] Verify custom prompt still works normally

### Test 7: Empty State

- [ ] Delete all custom prompts (if any exist)
- [ ] Verify dropdown shows "No prompts available"
- [ ] Apply a template → Save as Custom
- [ ] Verify first custom prompt created
- [ ] Verify dropdown now shows the custom prompt

---

## ✅ Visual Checks

- [ ] **Styling**: Template dropdown matches app design system
- [ ] **Icons**: Each template has appropriate emoji icon
- [ ] **Colors**: Selected template highlighted with primary color
- [ ] **Spacing**: Consistent padding and margins
- [ ] **Typography**: Text is readable (not too small/large)
- [ ] **Borders**: Subtle borders around dropdowns and previews
- [ ] **Animations**: Smooth dropdown open/close
- [ ] **Checkmark**: Selected template shows ✓ icon

---

## 🐛 Known Issues to Watch For

- [ ] Template not applying when clicked
- [ ] Dropdown not closing on outside click
- [ ] Keyboard navigation not working
- [ ] i18n keys missing (showing literal keys like "settings.postProcessing...")
- [ ] Template content not clearing properly
- [ ] Custom prompt workflow broken
- [ ] `${output}` placeholder not highlighted
- [ ] Blue template indicator not showing/clearing

---

## 📊 Test Results

**Date Tested**: _______________________
**Tester**: _______________________
**App Version**: 0.6.9

**Overall Status**:
- [ ] ✅ All tests passed
- [ ] ⚠️ Minor issues found (document below)
- [ ] ❌ Critical bugs found (document below)

**Issues Found**:
```
1.
2.
3.
```

**Notes**:
```


```

---

## 🎬 Next Steps After Testing

If all tests pass:
- [ ] Mark Task 3.2 as complete
- [ ] Proceed to Task 4.2: Accessibility audit
- [ ] Proceed to Task 5.1: Edge cases and error handling

If issues found:
- [ ] Document issues in test results above
- [ ] Create bug report
- [ ] Fix issues
- [ ] Re-test

---

## 💡 Tips for Testing

1. **Test with real data**: Try applying templates to actual transcription text
2. **Test different screen sizes**: Resize window to check responsiveness
3. **Test different languages**: Check if i18n works (if you have other locales enabled)
4. **Test performance**: Check if dropdown opens quickly with all 7 templates
5. **Test error scenarios**: Try clicking buttons rapidly, spam clicking, etc.

---

## 📝 Reference

Full test plan: `.claude/testing/prompt-templates-test-report.md`
Implementation details: `.claude/plan/prompt-templates.md`
