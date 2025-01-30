## Running the Applications

### Running the Client
To run the client app, use:
```sh
cd client
cargo r -r
```

### Running the Server
To run the server, use:
```sh
cd server
cargo r -r [OPTIONAL_PASSWORD]
```
- If you change the password, you must update it in web/server.ts as well

### Running the Web Application
The website runs on the port `8080`. To start it, use:
```sh
deno run --allow-net --allow-read --allow-write web/server.ts
```
If you don't have Deno installed, you can install it from [deno.land](https://deno.land/).
