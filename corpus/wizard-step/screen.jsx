export default function Wizard({ data, actions }) {
  return (
    <main aria-label="Setup wizard">
      <h1>Choose a plan</h1>
      <p>{data.progress}</p>
      <fieldset role="radiogroup">
        <legend>Plan</legend>
        <label>
          <input type="radio" name="plan" value="free" checked={data.plan === "free"} onChange={() => actions.set("plan", "free")} />
          <span>Free</span>
        </label>
        <label>
          <input type="radio" name="plan" value="pro" checked={data.plan === "pro"} onChange={() => actions.set("plan", "pro")} />
          <span>Pro</span>
        </label>
        <label>
          <input type="radio" name="plan" value="team" checked={data.plan === "team"} onChange={() => actions.set("plan", "team")} />
          <span>Team</span>
        </label>
      </fieldset>
      <div className="row gap-sm">
        <button type="button" onClick={() => actions.wizard.back()}>Back</button>
        <button type="button" data-variant="primary" disabled={!data.plan} onClick={() => actions.wizard.next()}>Next</button>
      </div>
    </main>
  );
}
