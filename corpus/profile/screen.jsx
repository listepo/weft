export default function Profile({ data, actions }) {
  return (
    <main aria-label="Profile">
      <img alt="Profile photo" src={data.user.avatar} />
      <h1>{data.user.name}</h1>
      <p>{data.user.bio}</p>
      <h2>Links</h2>
      <div className="row gap-md">
        <a href={data.user.website}>Website</a>
        <a href={data.user.github}>GitHub</a>
      </div>
      <button type="button" onClick={() => actions.profile.edit()}>Edit profile</button>
    </main>
  );
}
