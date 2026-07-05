import React from "react";
import { useTranslation } from "react-i18next";
import { Trash2 } from "lucide-react";

interface MeetingDeleteDialogProps {
  onCancel: () => void;
  onConfirm: () => void;
}

export const MeetingDeleteDialog: React.FC<MeetingDeleteDialogProps> = ({
  onCancel,
  onConfirm,
}) => {
  const { t } = useTranslation();

  return (
    <div className="fixed inset-0 bg-black/70 flex items-center justify-center z-[60]">
      <div className="bg-background border border-mid-gray/30 rounded-xl p-6 max-w-sm w-full mx-4">
        <div className="flex items-center gap-3 mb-4">
          <div className="p-2 bg-red-500/20 rounded-full">
            <Trash2 className="h-5 w-5 text-red-400" />
          </div>
          <h3 className="text-lg font-semibold">
            {t("meeting.detail.deleteTitle", "Delete Meeting")}
          </h3>
        </div>
        <p className="text-mid-gray mb-6">
          {t(
            "meeting.detail.confirmDelete",
            "Are you sure you want to delete this meeting? This action cannot be undone.",
          )}
        </p>
        <div className="flex gap-3 justify-end">
          <button
            type="button"
            onClick={onCancel}
            className="px-4 py-2 rounded-lg border border-mid-gray/30 hover:bg-mid-gray/20 transition-colors"
          >
            {t("common.cancel", "Cancel")}
          </button>
          <button
            type="button"
            onClick={onConfirm}
            className="px-4 py-2 rounded-lg bg-red-500 hover:bg-red-600 text-white transition-colors"
          >
            {t("common.delete", "Delete")}
          </button>
        </div>
      </div>
    </div>
  );
};

export default MeetingDeleteDialog;
