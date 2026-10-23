// Host-private factory. Its references never become plugin-visible globals.
(function (host, allowed) {
  'use strict';
  const parse = JSON.parse;
  const stringify = JSON.stringify;
  const freeze = Object.freeze;
  const handlers = new Map();
  const call = async (operation, args = {}) => {
    let input;
    try { input = stringify(args); }
    catch (_) { throw freeze({ code: 'invalid_input' }); }
    if (typeof input !== 'string') throw freeze({ code: 'invalid_input' });
    const result = parse(await host(operation, input));
    if (!result.ok) throw freeze(result.error);
    return result.value;
  };
  /** @plugin-api @since 2.0 @permission events.subscribe @stability stable */
  const on = (event, handler) => {
    if (!allowed.includes(event) || typeof handler !== 'function') throw freeze({ code: 'invalid_input' });
    const count = [...handlers.values()].reduce((n, list) => n + list.length, 0);
    if (count >= 64) throw freeze({ code: 'limit_exceeded' });
    const list = handlers.get(event) || [];
    const registration = { handler };
    list.push(registration);
    handlers.set(event, list);
    return () => { const index = list.indexOf(registration); if (index >= 0) list.splice(index, 1); };
  };
  const deepFreeze = (value) => {
    if (value && typeof value === 'object') { Object.values(value).forEach(deepFreeze); freeze(value); }
    return value;
  };
  const api = freeze({
    events: freeze({ on }),
    context: freeze({ current: () => call('context.current') }),
    identity: freeze({ current: context => call('identity.current', { context }) }),
    contest: freeze({
      current: context => call('contest.current', { context }),
      get: (context, contestId) => call('contest.get', { context, contestId }),
    }),
    submission: freeze({
      get: (context, contestId, submissionId) => call('submission.get', { context, contestId, submissionId }),
      list: (context, query) => call('submission.list', { context, query }),
    }),
    storage: freeze({
      get: key => call('storage.get', { key }),
      set: (key, value) => call('storage.set', { key, value }),
      delete: key => call('storage.delete', { key }),
      compareAndSet: (key, expected, value) => call('storage.compareAndSet', { key, expected, value }),
    }),
    http: freeze({ request: (context, request) => call('http.request', { context, request }) }),
    notification: freeze(Object.fromEntries(['info', 'success', 'warn', 'error'].map(level =>
      [level, message => call('notification.notify', { level, message })]))),
    log: freeze(Object.fromEntries(['debug', 'info', 'warn', 'error'].map(level =>
      [level, message => call('log.write', { level, message })]))),
  });
  const errorCode = error => {
    // Only a stable, host-known shape is reported; arbitrary thrown values stay opaque.
    if (!error || typeof error !== 'object') return 'script_error';
    const code = error.code;
    return typeof code === 'string' && /^[a-z][a-z_]{0,39}$/.test(code) ? code : 'script_error';
  };
  return {
    api,
    config: json => deepFreeze(parse(json)),
    // A snapshot preserves registration order and allows safe unregister during delivery.
    // One handler's rejection must not cancel the remaining handlers or later events.
    deliver: async json => {
      const envelope = deepFreeze(parse(json));
      let failed = 0;
      let code = '';
      for (const { handler } of [...(handlers.get(envelope.event.type) || [])]) {
        try { await handler(envelope.event, envelope); }
        catch (error) { failed += 1; if (!code) code = errorCode(error); }
      }
      return stringify({ failed, code });
    },
  };
})
