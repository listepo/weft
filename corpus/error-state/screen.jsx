export default function LoadError({ data, actions }) {
  return (
    <main aria-label="Load error" data-state="error">
      <div role="alert" data-tone="danger">
        <h2>Could not load your data</h2>
        <p>{data.error.message}</p>
      </div>
      <div className="row gap-sm">
        <button type="button" data-variant="primary" onClick={() => actions.load.retry()}>Retry</button>
        <a href="#" onClick={() => actions.nav.support()}>Contact support</a>
      </div>
    </main>
  );
}
