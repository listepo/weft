export default function Account({ data, actions }) {
  return (
    <main aria-label="Account">
      <h1>Account</h1>
      <div>
        <div role="tablist" aria-label="Account sections">
          <button role="tab" type="button" id="tab-overview" aria-selected={true} aria-controls="tab-overview-panel">Overview</button>
          <button role="tab" type="button" id="tab-billing" aria-selected={false} aria-controls="tab-billing-panel">Billing</button>
          <button role="tab" type="button" id="tab-team" aria-selected={false} aria-controls="tab-team-panel">Team</button>
        </div>
        <div role="tabpanel" id="tab-overview-panel" aria-labelledby="tab-overview">
          <p>{data.account.name}</p>
          <p>{data.account.email}</p>
        </div>
        <div role="tabpanel" id="tab-billing-panel" aria-labelledby="tab-billing" hidden>
          <p>{data.billing.plan}</p>
          <button type="button" data-variant="primary" onClick={() => actions.billing.change()}>Change plan</button>
        </div>
        <div role="tabpanel" id="tab-team-panel" aria-labelledby="tab-team" hidden>
          <ul>
            {data.team.map((member, memberIndex) => (
              <li key={memberIndex}>
                <p>{member.name}</p>
              </li>
            ))}
          </ul>
        </div>
      </div>
    </main>
  );
}
