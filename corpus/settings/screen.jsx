export default function Settings({ data, actions }) {
  return (
    <main aria-label="Settings">
      <h1>Settings</h1>
      <section aria-label="Notifications">
        <h2>Notifications</h2>
        <label>
          <input type="checkbox" role="switch" checked={data.emailAlerts} onChange={(e) => actions.set("emailAlerts", e.target.checked)} />
          <span>Email alerts</span>
        </label>
        <label>
          <input type="checkbox" role="switch" checked={data.pushAlerts} onChange={(e) => actions.set("pushAlerts", e.target.checked)} />
          <span>Push notifications</span>
        </label>
      </section>
      <section aria-label="Appearance">
        <h2>Appearance</h2>
        <label htmlFor="language">Language</label>
        <select id="language" value={data.language} onChange={(e) => actions.set("language", e.target.value)}>
          <option value="en">English</option>
          <option value="de">Deutsch</option>
          <option value="fr">Français</option>
        </select>
        <label>
          <input type="checkbox" role="switch" checked={data.darkMode} onChange={(e) => actions.set("darkMode", e.target.checked)} />
          <span>Dark mode</span>
        </label>
      </section>
      <button type="button" data-variant="primary" disabled={!data.dirty} onClick={() => actions.settings.save()}>Save changes</button>
    </main>
  );
}
