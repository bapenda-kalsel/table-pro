export const READ_ONLY_VERBS = ['SELECT', 'PRAGMA', 'EXPLAIN', 'WITH', 'VALUES'] as const;

/**
 * Strip leading `--` line comments and `/* *\/` block comments so the real
 * statement verb is what follows them.
 */
export function stripLeadingComments(sql: string): string {
  let s = sql.trimStart();
  for (;;) {
    if (s.startsWith('--')) {
      const nl = s.indexOf('\n');
      if (nl === -1) return '';
      s = s.slice(nl + 1).trimStart();
    } else if (s.startsWith('/*')) {
      const end = s.indexOf('*/', 2);
      if (end === -1) return '';
      s = s.slice(end + 2).trimStart();
    } else {
      return s;
    }
  }
}

/** The leading SQL verb (uppercase) of a statement, e.g. "select ..." -> "SELECT". */
export function firstVerb(text: string): string {
  const m = stripLeadingComments(text).match(/^[a-zA-Z]+/);
  return m ? m[0].toUpperCase() : 'OTHER';
}

/** True when the statement can mutate data or schema (i.e. not read-only). */
export function isWriteStatement(text: string): boolean {
  return !(READ_ONLY_VERBS as readonly string[]).includes(firstVerb(text));
}

/** Escape a single cell for CSV output per RFC 4180. */
export function csvCell(v: unknown): string {
  if (v === null || v === undefined) return '';
  const s = typeof v === 'object' ? JSON.stringify(v) : String(v);
  return /[",\n\r]/.test(s) ? `"${s.replace(/"/g, '""')}"` : s;
}

/** Render a query result as a CSV document (header row + data rows). */
export function rowsToCsv(columns: string[], rows: unknown[][]): string {
  const lines = [columns.map(csvCell).join(',')];
  for (const row of rows) {
    lines.push(row.map(csvCell).join(','));
  }
  return lines.join('\n');
}

/** Quote a SQL identifier, escaping embedded double quotes. */
export function qi(ident: string): string {
  return `"${ident.replace(/"/g, '""')}"`;
}

/** Render a JS value as a SQL literal (matching the Rust driver's rules). */
export function sqlLiteral(v: unknown): string {
  if (v === null || v === undefined) return 'NULL';
  if (typeof v === 'number') return String(v);
  if (typeof v === 'boolean') return v ? '1' : '0';
  if (typeof v === 'string') return `'${v.replace(/'/g, "''")}'`;
  return `'${JSON.stringify(v).replace(/'/g, "''")}'`;
}

/** Build an INSERT statement for a row object. */
export function buildInsertSql(table: string, row: Record<string, unknown>): string {
  const keys = Object.keys(row);
  const cols = keys.map(qi).join(', ');
  const vals = keys.map((k) => sqlLiteral(row[k])).join(', ');
  return `INSERT INTO ${qi(table)} (${cols}) VALUES (${vals});`;
}

/** Referential actions allowed in a FOREIGN KEY clause. */
export const FK_ACTIONS = ['NO ACTION', 'RESTRICT', 'CASCADE', 'SET NULL', 'SET DEFAULT'] as const;
export type FkAction = (typeof FK_ACTIONS)[number];

/** Whitelist a referential action; unknown values collapse to NO ACTION. */
export function sanitizeFkAction(a: string): FkAction {
  const up = a.trim().toUpperCase();
  return (FK_ACTIONS as readonly string[]).includes(up) ? (up as FkAction) : 'NO ACTION';
}

/** Preview matching the backend's `common::add_fk_sql` (ANSI dialect). */
export function buildAddFkSql(
  constraintName: string,
  table: string,
  column: string,
  refTable: string,
  refColumn: string,
  onDelete: string,
  onUpdate: string,
): string {
  return `ALTER TABLE ${qi(table)} ADD CONSTRAINT ${qi(constraintName)} FOREIGN KEY (${qi(column)}) REFERENCES ${qi(refTable)}(${qi(refColumn)}) ON DELETE ${sanitizeFkAction(onDelete)} ON UPDATE ${sanitizeFkAction(onUpdate)};`;
}

export interface ColumnDef {
  name: string;
  dataType: string;
  isPrimaryKey: boolean;
  isNotNull: boolean;
  isUnique: boolean;
  autoIncrement: boolean;
  defaultValue: string; // raw SQL literal/expression; '' means no default
}

/** SQLite types offered by the visual table designer. */
export const SQLITE_TYPES = [
  'INTEGER', 'TEXT', 'REAL', 'BLOB', 'NUMERIC', 'BOOLEAN', 'DATETIME', 'JSON',
] as const;

function isRawSqlLiteral(v: string): boolean {
  const t = v.trim();
  if (t === '') return false;
  // numbers, quoted strings, parenthesized exprs, function calls, keywords
  if (/^-?\d+(\.\d+)?$/.test(t)) return true;
  if (/^'.*'$/.test(t)) return true;
  if (/^\(.*\)$/.test(t)) return true;
  if (/^[a-zA-Z_]+\(.*\)$/.test(t)) return true;
  return ['NULL', 'TRUE', 'FALSE', 'CURRENT_TIMESTAMP', 'CURRENT_DATE', 'CURRENT_TIME'].includes(
    t.toUpperCase()
  );
}

/**
 * Build a CREATE TABLE statement from the designer's column list.
 * AUTOINCREMENT is only emitted for INTEGER PRIMARY KEY columns (SQLite rule).
 */
export function buildCreateTableSql(table: string, cols: ColumnDef[]): string {
  if (!table.trim()) throw new Error('Table name is required');
  const valid = cols.filter((c) => c.name.trim() !== '');
  if (valid.length === 0) throw new Error('At least one column is required');
  for (const c of valid) {
    if (!/^[a-zA-Z_][a-zA-Z0-9_]*$/.test(c.name.trim())) {
      throw new Error(`Invalid column name "${c.name}"`);
    }
  }

  const pkCount = valid.filter((c) => c.isPrimaryKey).length;
  const lines = valid.map((c) => {
    const parts = [qi(c.name.trim()), c.dataType || 'TEXT'];
    if (c.isPrimaryKey) {
      parts.push('PRIMARY KEY');
      // AUTOINCREMENT only valid on INTEGER PRIMARY KEY
      if (c.autoIncrement && (c.dataType || '').toUpperCase() === 'INTEGER') {
        parts.push('AUTOINCREMENT');
      }
    }
    if (c.isNotNull && !c.isPrimaryKey) parts.push('NOT NULL');
    if (c.isUnique && !c.isPrimaryKey) parts.push('UNIQUE');
    if (c.defaultValue.trim() !== '') {
      const d = c.defaultValue.trim();
      parts.push(`DEFAULT ${isRawSqlLiteral(d) ? d : `'${d.replace(/'/g, "''")}'`}`);
    }
    return `  ${parts.join(' ')}`;
  });

  // Composite PK -> table-level constraint
  if (pkCount > 1) {
    // strip inline PRIMARY KEYs, emit table-level PK
    const pkCols = valid.filter((c) => c.isPrimaryKey).map((c) => qi(c.name.trim()));
    const stripped = valid.map((c) => {
      const parts = [qi(c.name.trim()), c.dataType || 'TEXT'];
      if (c.isNotNull) parts.push('NOT NULL');
      if (c.isUnique) parts.push('UNIQUE');
      if (c.defaultValue.trim() !== '') {
        const d = c.defaultValue.trim();
        parts.push(`DEFAULT ${isRawSqlLiteral(d) ? d : `'${d.replace(/'/g, "''")}'`}`);
      }
      return `  ${parts.join(' ')}`;
    });
    stripped.push(`  PRIMARY KEY (${pkCols.join(', ')})`);
    return `CREATE TABLE ${qi(table.trim())} (\n${stripped.join(',\n')}\n);`;
  }

  return `CREATE TABLE ${qi(table.trim())} (\n${lines.join(',\n')}\n);`;
}
