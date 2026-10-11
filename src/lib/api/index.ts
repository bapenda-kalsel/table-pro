import { aiApi } from './ai';
import { auditApi } from './audit';
import { connectionsApi } from './connections';
import { dataApi } from './data';
import { opsApi } from './ops';
import { schemaApi } from './schema';
import { snippetsApi } from './snippets';

/**
 * Unified API facade — same `api.method()` shape as before, backed by
 * per-domain modules so each file stays small and focused.
 */
export const api = {
  ...connectionsApi,
  ...schemaApi,
  ...dataApi,
  ...opsApi,
  ...auditApi,
  ...snippetsApi,
  ...aiApi,
};
