import { For, Show, createSignal } from "solid-js";

interface Message {
  id: string;
  subject: string;
  unread: boolean;
}

interface Props {
  data: { messages: Message[]; filter: string; showArchived: boolean };
  actions: Record<string, (event?: unknown) => void>;
}

// A hand-written SolidJS screen in TypeScript.
export default function Inbox(props: Props) {
  const [open, setOpen] = createSignal(false);
  return (
    <main aria-label="Inbox">
      <h1>Inbox</h1>
      <input type="search" aria-label="Filter" value={props.data.filter} onInput={(e) => props.actions.set("filter", e.currentTarget.value)} />
      <ul>
        <Show when={props.data.messages.length > 0} fallback={<li>No messages</li>}>
          <For each={props.data.messages}>
            {(message) => (
              <li classList={{ unread: message.unread }}>
                <a href="#" onClick={() => props.actions["inbox.open"]()}>{message.subject}</a>
              </li>
            )}
          </For>
        </Show>
      </ul>
      <Show when={props.data.showArchived}>
        <section aria-label="Archive">
          <h2>Archive</h2>
        </section>
      </Show>
      <button type="button" onClick={() => setOpen(!open())}>
        Compose
      </button>
    </main>
  );
}
