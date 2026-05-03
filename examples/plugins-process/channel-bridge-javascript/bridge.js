#!/usr/bin/env node
function buildPayload(operation, payload) {
  switch (operation) {
    case 'send_message':
      return { accepted: true, target: payload.target ?? null };
    case 'receive_batch':
      return { messages: [] };
    case 'ack_inbound':
      return { acknowledged: payload.message_id ?? null };
    case 'complete_batch':
      return { completed: true, batch_cursor: payload.batch_cursor ?? null };
    default:
      return { error: `unsupported operation: ${operation}` };
  }
}
function emitResponse(line) {
  const trimmed = line.trim();
  if (!trimmed) return;
  const request = JSON.parse(trimmed);
  const response = {
    method: request.method ?? '',
    id: request.id ?? null,
    payload: buildPayload(request.payload?.operation ?? '', request.payload?.payload ?? {})
  };
  process.stdout.write(`${JSON.stringify(response)}\n`);
}
process.stdin.setEncoding('utf8');
let buffered = '';
process.stdin.on('data', chunk => {
  buffered += chunk;
  let newlineIndex = buffered.indexOf('\n');
  while (newlineIndex !== -1) {
    const line = buffered.slice(0, newlineIndex);
    buffered = buffered.slice(newlineIndex + 1);
    emitResponse(line);
    newlineIndex = buffered.indexOf('\n');
  }
});
process.stdin.on('end', () => {
  if (buffered.trim()) emitResponse(buffered);
});
process.stdin.resume();
