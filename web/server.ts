const PORT = 8080
const SOCKET = "/tmp/game-guess-a-word-socket"
const PASSWORD = "supersecret123"

class Connection {
  private socket: Deno.UnixConn | null = null
  private onConnectCallback: ((conn: Deno.UnixConn) => Promise<void>) | null = null
  onError: ((e: unknown) => void) | null = null

  private async callbacks() {
    if (this.socket && this.onConnectCallback) {
      try {
        await this.onConnectCallback(this.socket)
      } catch (e) {
        this.onError?.(e)
      }
    }
  }

  set onConnect(cb: ((conn: Deno.UnixConn) => Promise<void>) | null) {
    this.onConnectCallback = cb;
    if (this.socket && cb) {
      this.callbacks()
    }
  }

  get onConnect(): ((conn: Deno.UnixConn) => Promise<void>) | null {
    return this.onConnectCallback
  }

  constructor() {
    this.connect()
  }

  async connect() {
    if (this.socket) {
      this.socket.close()
      this.socket = null
    }
    await this.tryConnect()

    if (!this.socket) {
      setTimeout(() => this.connect(), 5000)
    } else {
      this.callbacks()
    }
  }

  private async tryConnect() {
    console.log("Connecting to server...")
    if (this.socket) {
      console.log("Socket already connected")
      return
    }

    try {
      this.socket = await Deno.connect({ transport: "unix", path: SOCKET })
      console.log("Connected to server")
    } catch (e) {
      console.log("Could not connect to server")
      console.error((e as Error).message)
    }
  }

  async writeBytes(bytes: Uint8Array) {
    if (!this.socket) throw new Error("Socket is not connected")
    let bytesWritten = 0

    while (bytesWritten < bytes.length) {
      const n = await this.socket.write(bytes.subarray(bytesWritten))
      if (n === null) throw new Error("Socket closed") 
      if (n) bytesWritten += n
    }
  }

  async readBytes(num: number): Promise<Uint8Array> {
    if (!this.socket) throw new Error("Socket is not connected")
    const bytes = new Uint8Array(num)
    let bytesRead = 0

    while (bytesRead < num) {
      const n = await this.socket.read(bytes.subarray(bytesRead))
      if (n === null) throw new Error("Socket closed") 
      if (n) bytesRead += n
    }

    return bytes
  }
}

type Game = {
  timestamp: number
  id: number
  hinter: number
  guesser: number
  word: string
  guesses: string[]
  hints: string[]
  winner: number | null
}

const connection = new Connection()
const games: Game[] = []

async function subscribeToGameUpdates() {
  const passBytes = new TextEncoder().encode(PASSWORD)
  const passLenBytes = new DataView(new ArrayBuffer(2))
  passLenBytes.setInt16(0, passBytes.length, false)

  const lenBytes = new DataView(new ArrayBuffer(2))
  lenBytes.setInt16(0, passBytes.length + passLenBytes.byteLength + 1, false)
  
  const bytes = new Uint8Array(2 + 1 + 2 + passBytes.buffer.byteLength)
  bytes.set(new Uint8Array(lenBytes.buffer), 0) // 2 bytes packet length
  bytes.set([254], 2) // 1 byte command type
  bytes.set(new Uint8Array(passLenBytes.buffer), 3) // 2 byte string length
  bytes.set(new Uint8Array(passBytes.buffer), 5) // password

  console.log("Subscribing to game updates...")
  await connection.writeBytes(bytes)
}

async function updateGames() {
  games.splice(0, games.length)
  while (true) {
    console.log("Waiting for data to update games...")

    const lenBytes = await connection.readBytes(2)
    const len = new DataView(lenBytes.buffer).getUint16(0, false)
    const dataBytes = await connection.readBytes(len)
    const stringJson = new TextDecoder().decode(dataBytes.buffer.slice(3)) // skip the byte type and length

    const json = JSON.parse(stringJson) as Game;
    console.log("Received game with id:", json.id)

    games.push({
      timestamp: json.timestamp * 1000,
      id: json.id,
      hinter: json.hinter,
      guesser: json.guesser,
      word: json.word,
      guesses: json.guesses,
      hints: json.hints,
      winner: json.winner,
    })
  }
}

connection.onConnect = async () => {
  const read = await connection.readBytes(3);
  if (read[2] !== 2) throw new Error("Invalid first message")
  await subscribeToGameUpdates()
  await updateGames()
}

connection.onError = (e: unknown) => {
  console.error("Connection error:", (e as Error)?.message)

  setTimeout(() => {
    connection.connect()
  }, 5000)
}

Deno.serve({ port: PORT }, (req) => {
  const url = new URL(req.url);
  console.log("Request Path:", url.pathname);

  // static files
  if (url.pathname === "/") {
    return serveFile("index.html", "text/html");
  }
  if (url.pathname === "/main.js") {
    return serveFile("main.js", "application/javascript");
  }
  if (url.pathname === "/style.css") {
    return serveFile("style.css", "text/css");
  }

  // API
  if (url.pathname === "/games" && req.method === "GET") {
    return new Response(JSON.stringify({ games }), {
      headers: { "Content-Type": "application/json" },
    });
  }

  return new Response("404 Not Found", { status: 404 });
});

async function serveFile(filePath: string, contentType: string) {
  try {
    const file = await Deno.readFile(filePath);
    return new Response(file, { headers: { "Content-Type": contentType } });
  } catch {
    return new Response("File Not Found", { status: 404 });
  }
}
