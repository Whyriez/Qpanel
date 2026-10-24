import { useTranslation } from "react-i18next";

export function Brand() {
  const { t } = useTranslation();
  return (
    <div className="panel-brand" aria-label="QonPanel">
      <img className="brand-mark" src="/brand/qonpanel-mark.svg" width="40" height="40" alt="" aria-hidden="true" />
      <span className="brand-name">
        QonPanel
        <span className="brand-description">{t("design.hostingPanel")}</span>
      </span>
    </div>
  );
}

export function BrandLoading({ message, fullScreen = false }: { message: string; fullScreen?: boolean }) {
  return <div className={`brand-loading${fullScreen ? ' brand-loading--full' : ''}`} role="status" aria-live="polite">
    <Brand />
    <div className="brand-loading__line" aria-hidden="true" />
    <p>{message}</p>
  </div>;
}
