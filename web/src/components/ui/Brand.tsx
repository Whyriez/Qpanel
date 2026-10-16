import { useTranslation } from "react-i18next";

export function Brand() {
  const { t } = useTranslation();
  return (
    <div className="panel-brand" aria-label="Qpanel">
      <svg
        className="brand-mark"
        viewBox="0 0 36 36"
        fill="none"
        aria-hidden="true"
      >
        <rect width="36" height="36" rx="9" fill="currentColor" />
        <circle cx="17" cy="17" r="7" stroke="var(--primary-foreground)" strokeWidth="3.5" />
        <path d="m21 21 6 6" stroke="var(--primary-foreground)" strokeWidth="3.5" strokeLinecap="round" />
      </svg>
      <span className="brand-name">
        Qpanel
        <span className="brand-description">{t("design.hostingPanel")}</span>
      </span>
    </div>
  );
}
