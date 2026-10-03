export default function Login({ data, actions }) {
  return (
    <main aria-label="Sign in">
      <form data-state="idle" onSubmit={(e) => { e.preventDefault(); actions.auth.submit(); }}>
        <h1>Sign in</h1>
        <div className="stack gap-md">
          <label htmlFor="email">Email</label>
          <input id="email" type="email" required value={data.email} onChange={(e) => actions.set("email", e.target.value)} />
          <label htmlFor="password">Password</label>
          <input id="password" type="password" required value={data.password} onChange={(e) => actions.set("password", e.target.value)} />
        </div>
        <button type="submit" data-variant="primary" disabled={!data.email}>Sign in</button>
        <footer>
          <a href="#" onClick={() => actions.nav.reset()}>Forgot password?</a>
          <a href="#" onClick={() => actions.nav.signup()}>Create an account</a>
        </footer>
      </form>
    </main>
  );
}
