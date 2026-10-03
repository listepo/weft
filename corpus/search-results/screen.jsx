export default function Search({ data, actions }) {
  return (
    <main aria-label="Search">
      <form onSubmit={(e) => { e.preventDefault(); actions.search.run(); }}>
        <div className="row gap-sm">
          <label htmlFor="query">Search</label>
          <input id="query" type="search" value={data.query} onChange={(e) => actions.set("query", e.target.value)} />
          <button type="submit" data-variant="primary">Search</button>
        </div>
      </form>
      {data.hasResults && (
        <ul>
          {data.results.map((result, resultIndex) => (
            <li key={resultIndex}>
              <h3>{result.title}</h3>
              <p>{result.snippet}</p>
              <a href="#" onClick={() => actions.results.open()}>Open</a>
            </li>
          ))}
        </ul>
      )}
      {!data.hasResults && (
        <p>No results found</p>
      )}
    </main>
  );
}
