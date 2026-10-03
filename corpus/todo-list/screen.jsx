export default function Todos({ data, actions }) {
  return (
    <main aria-label="Tasks">
      <h1>Tasks</h1>
      <form onSubmit={(e) => { e.preventDefault(); actions.todo.add(); }}>
        <div className="row gap-sm">
          <label htmlFor="draft">New task</label>
          <input id="draft" type="text" value={data.draft} onChange={(e) => actions.set("draft", e.target.value)} />
          <button type="submit" data-variant="primary" disabled={!data.draft}>Add</button>
        </div>
      </form>
      <ul>
        {data.todos.map((todo, todoIndex) => (
          <li key={todoIndex}>
            <label>
              <input type="checkbox" checked={todo.done} onChange={(e) => actions.set(`todos.${todoIndex}.done`, e.target.checked)} />
              <span>{todo.title}</span>
            </label>
            <button type="button" onClick={() => actions.todo.remove()}>Remove</button>
          </li>
        ))}
      </ul>
      <div className="row gap-xs">
        <p>{data.remaining}</p>
        <p>items left</p>
      </div>
    </main>
  );
}
