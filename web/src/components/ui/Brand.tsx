import { useTranslation } from "react-i18next";

export function Brand() {
  const { t } = useTranslation();
  return (
    <div className="panel-brand" aria-label="Nusa Panel">
      <svg
        className="brand-mark"
        viewBox="0 0 36 36"
        fill="none"
        aria-hidden="true"
      >
        <rect width="36" height="36" rx="9" fill="currentColor" />
        <path
          d="M11 25V11h4l6 9v-9h4v14h-4l-6-9v9h-4Z"
          fill="var(--primary-foreground)"
        />
      </svg>
      <span className="brand-name">
        Nusa
        <span className="brand-description">{t("design.hostingPanel")}</span>
      </span>
    </div>
  );
}
