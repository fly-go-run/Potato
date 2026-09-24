import test from 'node:test';
import assert from 'node:assert/strict';
import { curatedChatModels } from '../scripts/cloud-model-policy.mjs';
import { cloudCatalog, cloudConfiguration, cloudRoute, validateCloudThinking } from '../src/cloud.ts';

test('curated cloud catalog contains only the requested chat models and exact verified efforts', () => {
  const providers = ['deepseek', 'sub2api'].map(id => ({ id, name: id, endpoint: `https://${id}.invalid/v1/chat/completions`, api_key: 'synthetic-secret', models: curatedChatModels(id) }));
  const config = cloudConfiguration(JSON.stringify({ providers, default_model: 'deepseek/deepseek-flash' }));
  const catalog = cloudCatalog(config);
  assert.deepEqual(catalog.data.map(m => m.id), ['deepseek/deepseek-flash', 'sub2api/gpt-5.6-sol', 'sub2api/gpt-6']);
  assert.deepEqual(catalog.data.map(m => m.name), ['DeepSeek V4.1 Flash', 'GPT-5.6 Sol', 'GPT-6']);
  assert.deepEqual(catalog.data[0].reasoning_effort_options, ['low', 'high', 'max']);
  assert.deepEqual(catalog.data[1].reasoning_effort_options, ['none', 'low', 'medium', 'high', 'xhigh', 'max']);
  assert.deepEqual(catalog.data[2].reasoning_effort_options, ['low']);
  assert.equal(cloudRoute(config, 'sub2api/gpt-5.6-sol').model.id, 'gpt-5.6-sol');
  const gpt6 = cloudRoute(config, 'sub2api/gpt-6');
  assert.equal(gpt6.model.id, 'gpt-6');
  assert.doesNotThrow(() => validateCloudThinking(gpt6.provider, gpt6.model, {}));
  assert.doesNotThrow(() => validateCloudThinking(gpt6.provider, gpt6.model, { reasoning_effort: 'low' }));
  assert.throws(() => validateCloudThinking(gpt6.provider, gpt6.model, { reasoning_effort: 'high' }));
  for (const id of ['deepseek/deepseek-v4-pro', 'deepseek/deepseek-v4.1-flash', 'sub2api/gpt-5.5', 'sub2api/gpt-5.6', 'sub2api/gpt-image-2']) assert.throws(() => cloudRoute(config, id));
  for (const provider of config.providers) {
    const model = provider.models[0];
    for (const effort of model.reasoning_effort_options!) assert.doesNotThrow(() => validateCloudThinking(provider, model, { reasoning_effort: effort }));
    assert.throws(() => validateCloudThinking(provider, model, { reasoning_effort: 'potato_invalid' }));
  }
  assert.throws(() => validateCloudThinking(config.providers[0], config.providers[0].models[0], { reasoning_effort: 'medium' }));
  assert.throws(() => validateCloudThinking(config.providers[1], config.providers[1].models[0], { thinking: { type: 'enabled' } }));
});
