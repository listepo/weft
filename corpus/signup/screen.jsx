export default function Signup({ data, actions }) {
  return (
    <main aria-label="Create account">
      <form data-state="idle" onSubmit={(e) => { e.preventDefault(); actions.auth.signup(); }}>
        <h1>Create your account</h1>
        <div className="stack gap-md">
          <label htmlFor="name">Full name</label>
          <input id="name" type="text" required value={data.name} onChange={(e) => actions.set("name", e.target.value)} />
          <label htmlFor="email">Email</label>
          <input id="email" type="email" required value={data.email} onChange={(e) => actions.set("email", e.target.value)} />
          <label htmlFor="password">Password</label>
          <input id="password" type="password" required value={data.password} onChange={(e) => actions.set("password", e.target.value)} />
          <label htmlFor="confirm">Confirm password</label>
          <input id="confirm" type="password" required value={data.confirm} onChange={(e) => actions.set("confirm", e.target.value)} />
          <label>
            <input type="checkbox" checked={data.acceptTerms} onChange={(e) => actions.set("acceptTerms", e.target.checked)} />
            <span>I accept the terms of service</span>
          </label>
        </div>
        <button type="submit" data-variant="primary" disabled={!data.acceptTerms}>Create account</button>
        <footer>
          <a href="#" onClick={() => actions.nav.signin()}>Already have an account? Sign in</a>
        </footer>
      </form>
    </main>
  );
}
