import { useEffect, useState } from "react";
import { RpcClient } from "./rpc";
import type { ConnectionId } from "./types/ConnectionId";

// Walking-skeleton UI: open one TCP connection, read holding registers, and
// show the values. Subscription/live tables, tag decoding, and charts arrive
// in later increments.

export function App() {
  const [rpc, setRpc] = useState<RpcClient | null>(null);
  const [connError, setConnError] = useState<string | null>(null);

  const [host, setHost] = useState("127.0.0.1");
  const [port, setPort] = useState("502");
  const [connectionId, setConnectionId] = useState<ConnectionId | null>(null);

  const [slave, setSlave] = useState("1");
  const [address, setAddress] = useState("0");
  const [quantity, setQuantity] = useState("10");
  const [values, setValues] = useState<number[]>([]);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    const client = new RpcClient();
    client
      .ready()
      .then(() => setRpc(client))
      .catch((e) => setConnError(String(e)));
  }, []);

  async function createConnection() {
    if (!rpc) return;
    setError(null);
    try {
      const id = await rpc.createConnection({
        name: `${host}:${port}`,
        transport: { type: "tcp", host, port: Number(port) },
      });
      setConnectionId(id);
    } catch (e) {
      setError(String(e));
    }
  }

  async function read() {
    if (!rpc || !connectionId) return;
    setBusy(true);
    setError(null);
    try {
      const result = await rpc.readHoldingRegisters({
        connection: connectionId,
        slave: Number(slave),
        address: Number(address),
        quantity: Number(quantity),
      });
      setValues(result);
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }

  if (connError) return <p>Backend WebSocket unavailable: {connError}</p>;
  if (!rpc) return <p>Connecting to backend…</p>;

  return (
    <main style={{ fontFamily: "system-ui, sans-serif", maxWidth: 720, padding: "2rem" }}>
      <h1>Modbus Client</h1>

      <section>
        <h2>Connection</h2>
        <label>
          host <input value={host} onChange={(e) => setHost(e.target.value)} />
        </label>{" "}
        <label>
          port <input value={port} onChange={(e) => setPort(e.target.value)} size={6} /></label>{" "}
        <button onClick={createConnection} disabled={!rpc}>
          open
        </button>
        {connectionId && <p>connection id: <code>{connectionId}</code></p>}
      </section>

      <section>
        <h2>Read holding registers</h2>
        <label>slave <input value={slave} onChange={(e) => setSlave(e.target.value)} size={4} /></label>{" "}
        <label>address <input value={address} onChange={(e) => setAddress(e.target.value)} size={6} /></label>{" "}
        <label>quantity <input value={quantity} onChange={(e) => setQuantity(e.target.value)} size={4} /></label>{" "}
        <button onClick={read} disabled={!connectionId || busy}>
          {busy ? "reading…" : "read"}
        </button>
        {error && <p style={{ color: "crimson" }}>{error}</p>}
        {values.length > 0 && (
          <table border={1} cellPadding={4} style={{ borderCollapse: "collapse", marginTop: 8 }}>
            <thead>
              <tr>
                {values.map((_, i) => (
                  <th key={i}>{Number(address) + i}</th>
                ))}
              </tr>
            </thead>
            <tbody>
              <tr>
                {values.map((v, i) => (
                  <td key={i}>0x{v.toString(16).padStart(4, "0")}</td>
                ))}
              </tr>
            </tbody>
          </table>
        )}
      </section>
    </main>
  );
}
