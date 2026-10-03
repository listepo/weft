export default function Files({ data, actions }) {
  return (
    <main aria-label="File">
      <h1>Quarterly report</h1>
      <button type="button" data-variant="danger" onClick={() => actions.dialog.open()}>Delete file</button>
      <dialog aria-label="Delete file?" open={data.confirmOpen} onClose={() => actions.dialog.close()}>
        <p>This cannot be undone.</p>
        <p>{data.file.name}</p>
        <footer>
          <button type="button" onClick={() => actions.dialog.close()}>Cancel</button>
          <button type="button" data-variant="danger" onClick={() => actions.file.delete()}>Delete</button>
        </footer>
      </dialog>
    </main>
  );
}
