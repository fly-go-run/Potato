export { ChatJob } from './chat-job';
export { CloudModelSettings } from './cloud-model-store';
import { handleRemoteAccount } from './remote-auth';
import chat from './index';
import { handleRemote, response } from './remote';
export { RemoteDevice } from './remote';
export { RemoteAccount, RemoteLogin } from './remote-auth';
export default {
  async fetch(request, env, ctx) {
    if (new URL(request.url).pathname.startsWith('/v1/cloud/')) {
      if (!(await env.REMOTE_RATE_LIMIT.limit({ key: 'cloud:' + (request.headers.get('cf-connecting-ip') ?? 'local') })).success) return response({ error: '请求过多，请稍后重试' }, 429);
      return await handleRemoteAccount(request, env) ?? response({ error: 'Not found' }, 404);
    }
    if (new URL(request.url).pathname.startsWith('/v1/remote/')) {
      if (String(env.REMOTE_CONTROL_ENABLED) === 'false') return response({ error: '远程服务暂时关闭，请稍后重试' }, 503);
      return handleRemote(request, env);
    }
    return chat.fetch(request, env);
  }
} satisfies ExportedHandler<Env>;
