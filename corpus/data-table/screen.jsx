export default function Users({ data, actions }) {
  return (
    <main aria-label="Users">
      <h1>Users</h1>
      <table aria-label="Users">
        <thead>
          <tr>
            <th aria-sort="ascending"><button type="button" onClick={() => actions.users.sort()}>Name</button></th>
            <th aria-sort="none"><button type="button" onClick={() => actions.users.sort()}>Email</button></th>
            <th aria-sort="none"><button type="button" onClick={() => actions.users.sort()}>Role</button></th>
            <th>Actions</th>
          </tr>
        </thead>
        <tbody>
          {data.users.map((user, userIndex) => (
            <tr key={userIndex}>
              <td>
                <p>{user.name}</p>
              </td>
              <td>
                <p>{user.email}</p>
              </td>
              <td>
                <p>{user.role}</p>
              </td>
              <td>
                <button type="button" data-variant="danger" onClick={() => actions.users.delete()}>Delete</button>
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </main>
  );
}
