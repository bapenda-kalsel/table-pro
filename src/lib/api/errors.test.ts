import { describe, expect, it } from 'vitest';
import { ApiError, apiErrorMessage, normalizeError } from './errors';

describe('normalizeError', () => {
  it('maps tagged backend errors to ApiError with code', () => {
    const raw = { code: 'tls_config', message: 'TLS mode is required' };
    const err = normalizeError(raw);
    expect(err).toBeInstanceOf(ApiError);
    expect(err.code).toBe('tls_config');
    expect(err.message).toBe('TLS mode is required');
    expect(String(err)).toBe('TLS mode is required');
  });

  it('maps plain string rejections to ApiError without code', () => {
    const err = normalizeError('not connected to database');
    expect(err.code).toBeNull();
    expect(err.message).toBe('not connected to database');
  });

  it('keeps ApiError instances as-is', () => {
    const err = new ApiError('database', 'x', null);
    expect(normalizeError(err)).toBe(err);
  });

  it('extracts message from generic objects instead of [object Object]', () => {
    expect(apiErrorMessage({ message: 'boom' })).toBe('boom');
    expect(apiErrorMessage({})).toBe('[object Object]');
    expect(apiErrorMessage(null)).toBe('null');
  });

  it('wraps unknown values with a readable message', () => {
    expect(normalizeError(42).message).toBe('42');
  });
});
