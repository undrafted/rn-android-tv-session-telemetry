// Local, deterministic endpoint for the TV network scenario. No external service or user data.
const http = require('node:http');
const { Buffer } = require('node:buffer');
const server = http.createServer((request, response) => {
  const body = JSON.stringify({ items: ['card-1', 'card-2'] });
  response.writeHead(200, {
    'Content-Type': 'application/json',
    'Content-Length': Buffer.byteLength(body),
  });
  response.end(body);
});
server.listen(8787, '127.0.0.1', () => {
  process.stdout.write('Network fixture listening on 127.0.0.1:8787\n');
});
