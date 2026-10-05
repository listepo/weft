import { useState } from "react";
import { Price } from "./price";

// A hand-written React screen: most of it reads by convention, the rest is listed as losses.
export default function Cart({ data, actions, ...rest }) {
  const [coupon, setCoupon] = useState("");
  const total = data.total;
  return (
    <main aria-label="Cart" {...rest}>
      <h1>Your cart</h1>
      {data.items.length === 0 ? (
        <p>Your cart is empty.</p>
      ) : (
        <ul className="stack gap-sm">
          {data.items.map(({ name, price }, index) => (
            <li key={index} style={{ padding: 8, opacity: 1 }}>
              <span>{name}</span>
              <Price value={price} />
              <button type="button" onClick={() => actions.cart.remove()}>
                Remove
              </button>
            </li>
          ))}
        </ul>
      )}
      <p>{`Total: ${total}`}</p>
      {data.loggedIn && <p>Signed in as {data.user.name}</p>}
      {coupon ? <p>Coupon applied</p> : null}
      <fieldset role="radiogroup" aria-label="Delivery">
        <legend>Delivery</legend>
        <label>
          <input type="radio" name="delivery" value="standard" checked={data.delivery === "standard"} onChange={() => actions.set("delivery", "standard")} />
          Standard
        </label>
        <label>
          <input type="radio" name="delivery" value="express" checked={data.delivery === "express"} onChange={() => actions.set("delivery", "express")} />
          Express
        </label>
      </fieldset>
      <label>
        Coupon
        <input type="text" value={coupon} onChange={(e) => setCoupon(e.target.value)} />
      </label>
      <button type="button" data-variant="primary" disabled={!data.items.length} onClick={actions.cart.checkout}>
        Check out
      </button>
    </main>
  );
}
