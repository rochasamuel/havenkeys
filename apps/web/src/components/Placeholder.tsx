import { Icon } from "./Icon";

/*
 * A desktop-app window waiting for its screenshot. The skeleton mirrors the
 * real vault layout (sidebar, item list, detail) so the composition around it
 * holds; swap the whole component for an <img> once a capture exists.
 */
export function DesktopPlaceholder({ label, note }: { label: string; note: string }) {
  return (
    <figure className="window window--placeholder">
      <div className="window__bar" aria-hidden="true">
        <span />
        <span />
        <span />
        <em>HavenKeys</em>
      </div>
      <div className="skeleton" aria-hidden="true">
        <div className="skeleton__side">
          <i className="w60" />
          <i className="w80" />
          <i className="w70" />
          <i className="w50" />
        </div>
        <div className="skeleton__list">
          {Array.from({ length: 7 }, (_, n) => (
            <div className="skeleton__row" key={n}>
              <b />
              <span>
                <i className={n % 2 ? "w70" : "w50"} />
                <i className="w40 dim" />
              </span>
            </div>
          ))}
        </div>
        <div className="skeleton__detail">
          <i className="w40 tall" />
          <i className="w60 dim" />
          <div className="skeleton__field">
            <i className="w30 dim" />
            <i className="w70" />
          </div>
          <div className="skeleton__field">
            <i className="w30 dim" />
            <i className="w50" />
          </div>
          <div className="skeleton__field">
            <i className="w30 dim" />
            <i className="w40 brass" />
          </div>
        </div>
      </div>
      <figcaption className="placeholder__tag">
        <Icon name="image" size={16} />
        <span>
          <strong>{label}</strong>
          {note}
        </span>
      </figcaption>
    </figure>
  );
}
