export default function AccountMenu({ data, actions }) {
  return (
    <main aria-label="Account menu">
      <p>{data.user.email}</p>
      <div role="menu" aria-label="Account">
        <button role="menuitem" type="button" onClick={() => actions.nav.profile()}>Profile</button>
        <button role="menuitem" type="button" onClick={() => actions.nav.settings()}>Settings</button>
        <button role="menuitem" type="button" disabled={!data.isOwner} onClick={() => actions.nav.billing()}>Billing</button>
        <button role="menuitem" type="button" onClick={() => actions.auth.signout()}>Sign out</button>
      </div>
    </main>
  );
}
