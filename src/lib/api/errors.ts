/**
 * Normalized API error. The backend serializes `AppError` as a tagged
 * object (`{ code, message, ... }`); older/other rejections may be plain
 * strings. `ApiError` unifies both so `catch (e)` always gets a usable
 * `message` and an optional machine-readable `code`.
 */

export type ApiErrorCode =
  | 'validation'
  | 'not_connected'
  | 'not_found'
  | 'credential_unavailable'
  | 'tls_config'
  | 'connection'
  | 'timeout'
  | 'database'
  | 'storage'
  | 'storage_corrupt'
  | 'busy'
  | 'stale_operation'
  | 'confirmation_required'
  | 'confirmation_invalid'
  | 'unsupported'
  | 'internal';

export class ApiError extends Error {
  readonly code: ApiErrorCode | null;
  readonly raw: unknown;

  constructor(code: ApiErrorCode | null, message: string, raw: unknown) {
    super(message);
    this.name = 'ApiError';
    this.code = code;
    this.raw = raw;
  }

  override toString(): string {
    return this.message;
  }
}

/** Extract a human-readable message from any invoke rejection. */
export function apiErrorMessage(err: unknown): string {
  if (err instanceof ApiError) return err.message;
  if (err && typeof err === 'object' && 'message' in err) {
    return String((err as { message: unknown }).message);
  }
  return String(err);
}

/** Normalize any invoke rejection into an ApiError. */
export function normalizeError(err: unknown): ApiError {
  if (err instanceof ApiError) return err;
  if (err && typeof err === 'object' && 'code' in err && 'message' in err) {
    const e = err as { code: string; message: unknown };
    return new ApiError(e.code as ApiErrorCode, String(e.message), err);
  }
  if (typeof err === 'string') {
    return new ApiError(null, err, err);
  }
  return new ApiError(null, apiErrorMessage(err), err);
}
