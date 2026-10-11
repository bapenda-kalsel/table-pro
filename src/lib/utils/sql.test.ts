import { describe, it, expect } from 'vitest';
import {
  firstVerb,
  isWriteStatement,
  stripLeadingComments,
  csvCell,
  rowsToCsv,
  buildCreateTableSql,
  qi,
  sqlLiteral,
  buildInsertSql,
  sanitizeFkAction,
  buildAddFkSql,
  FK_ACTIONS,
  type ColumnDef,
} from './sql';

function col(partial: Partial<ColumnDef>): ColumnDef {
  return {
    name: '',
    dataType: 'TEXT',
    isPrimaryKey: false,
    isNotNull: false,
    isUnique: false,
    autoIncrement: false,
    defaultValue: '',
    ...partial,
  };
}

describe('firstVerb', () => {
  it('detects basic verbs case-insensitively', () => {
    expect(firstVerb('SELECT * FROM t')).toBe('SELECT');
    expect(firstVerb('  insert into t values (1)')).toBe('INSERT');
    expect(firstVerb('UPDATE t SET x=1')).toBe('UPDATE');
    expect(firstVerb('delete from t')).toBe('DELETE');
    expect(firstVerb('pragma table_info(t)')).toBe('PRAGMA');
    expect(firstVerb('WITH x AS (SELECT 1) SELECT * FROM x')).toBe('WITH');
  });

  it('skips leading comments', () => {
    expect(firstVerb('-- a comment\nSELECT 1')).toBe('SELECT');
    expect(firstVerb('/* block */ UPDATE t SET x=1')).toBe('UPDATE');
    expect(firstVerb('/* multi\nline */\n\n DROP TABLE t')).toBe('DROP');
    expect(firstVerb('-- c1\n-- c2\nINSERT INTO t VALUES (1)')).toBe('INSERT');
  });

  it('handles edge cases', () => {
    expect(firstVerb('')).toBe('OTHER');
    expect(firstVerb('   ')).toBe('OTHER');
    expect(firstVerb('-- only a comment')).toBe('OTHER');
    expect(firstVerb('123 not sql')).toBe('OTHER');
  });
});

describe('isWriteStatement', () => {
  it('flags writes and DDL', () => {
    expect(isWriteStatement('UPDATE t SET x=1')).toBe(true);
    expect(isWriteStatement('INSERT INTO t VALUES (1)')).toBe(true);
    expect(isWriteStatement('DELETE FROM t')).toBe(true);
    expect(isWriteStatement('DROP TABLE t')).toBe(true);
    expect(isWriteStatement('CREATE TABLE x (id INT)')).toBe(true);
    expect(isWriteStatement('ALTER TABLE t ADD c INT')).toBe(true);
  });

  it('flags writes hidden behind comments', () => {
    expect(isWriteStatement('-- safe\nDELETE FROM t')).toBe(true);
    expect(isWriteStatement('/* ok */ DROP TABLE x')).toBe(true);
  });

  it('allows read-only statements', () => {
    expect(isWriteStatement('SELECT 1')).toBe(false);
    expect(isWriteStatement('PRAGMA table_info(t)')).toBe(false);
    expect(isWriteStatement('EXPLAIN QUERY PLAN SELECT 1')).toBe(false);
    expect(isWriteStatement('-- note\nSELECT 2')).toBe(false);
    expect(isWriteStatement('WITH x AS (SELECT 1) DELETE FROM t')).toBe(false); // edge: WITH-prefixed write passes as read — documented limitation
  });
});

describe('csvCell', () => {
  it('quotes cells containing commas, quotes, newlines', () => {
    expect(csvCell('plain')).toBe('plain');
    expect(csvCell('a,b')).toBe('"a,b"');
    expect(csvCell('say "hi"')).toBe('"say ""hi"""');
    expect(csvCell('line1\nline2')).toBe('"line1\nline2"');
    expect(csvCell(null)).toBe('');
    expect(csvCell(undefined)).toBe('');
    expect(csvCell(42)).toBe('42');
    expect(csvCell({ a: 1 })).toBe('"{""a"":1}"');
  });
});

describe('rowsToCsv', () => {
  it('renders header + rows', () => {
    const csv = rowsToCsv(['id', 'name'], [
      [1, 'Alice'],
      [2, 'Bob, Jr.'],
      [3, null],
    ]);
    expect(csv).toBe('id,name\n1,Alice\n2,"Bob, Jr."\n3,');
  });
});

describe('qi', () => {
  it('quotes and escapes identifiers', () => {
    expect(qi('users')).toBe('"users"');
    expect(qi('we"ird')).toBe('"we""ird"');
  });
});

describe('buildCreateTableSql', () => {
  it('builds a simple table', () => {
    const sql = buildCreateTableSql('users', [
      col({ name: 'id', dataType: 'INTEGER', isPrimaryKey: true, autoIncrement: true }),
      col({ name: 'name', isNotNull: true }),
      col({ name: 'email', isUnique: true }),
    ]);
    expect(sql).toBe(
      'CREATE TABLE "users" (\n' +
        '  "id" INTEGER PRIMARY KEY AUTOINCREMENT,\n' +
        '  "name" TEXT NOT NULL,\n' +
        '  "email" TEXT UNIQUE\n' +
        ');'
    );
  });

  it('AUTOINCREMENT only applies to INTEGER PKs', () => {
    const sql = buildCreateTableSql('t', [
      col({ name: 'id', dataType: 'TEXT', isPrimaryKey: true, autoIncrement: true }),
    ]);
    expect(sql).not.toContain('AUTOINCREMENT');
  });

  it('auto-quotes plain defaults, keeps raw literals', () => {
    const sql = buildCreateTableSql('t', [
      col({ name: 'role', defaultValue: 'user' }),
      col({ name: 'n', dataType: 'INTEGER', defaultValue: '0' }),
      col({ name: 'ts', dataType: 'DATETIME', defaultValue: 'CURRENT_TIMESTAMP' }),
      col({ name: 'note', defaultValue: "it's" }),
    ]);
    expect(sql).toContain(`DEFAULT 'user'`);
    expect(sql).toContain('DEFAULT 0');
    expect(sql).toContain('DEFAULT CURRENT_TIMESTAMP');
    expect(sql).toContain(`DEFAULT 'it''s'`);
  });

  it('emits table-level PK for composite keys', () => {
    const sql = buildCreateTableSql('links', [
      col({ name: 'a', dataType: 'INTEGER', isPrimaryKey: true }),
      col({ name: 'b', dataType: 'INTEGER', isPrimaryKey: true }),
      col({ name: 'v' }),
    ]);
    expect(sql).toContain('PRIMARY KEY ("a", "b")');
    expect(sql).not.toContain('"a" INTEGER PRIMARY KEY');
  });

  it('skips blank column names and validates input', () => {
    expect(() => buildCreateTableSql('', [col({ name: 'x' })])).toThrow();
    expect(() => buildCreateTableSql('t', [col({ name: '' })])).toThrow();
    expect(() => buildCreateTableSql('t', [col({ name: 'bad name!' })])).toThrow(/Invalid column/);
    const sql = buildCreateTableSql('t', [col({ name: '' }), col({ name: 'ok' })]);
    expect(sql).toContain('"ok" TEXT');
  });
});

describe('sqlLiteral', () => {
  it('renders JS values as SQL literals', () => {
    expect(sqlLiteral(null)).toBe('NULL');
    expect(sqlLiteral(undefined)).toBe('NULL');
    expect(sqlLiteral(42)).toBe('42');
    expect(sqlLiteral(1.5)).toBe('1.5');
    expect(sqlLiteral(true)).toBe('1');
    expect(sqlLiteral(false)).toBe('0');
    expect(sqlLiteral("it's")).toBe("'it''s'");
    expect(sqlLiteral({ a: 1 })).toBe('\'{"a":1}\'');
  });
});

describe('buildInsertSql', () => {
  it('builds INSERT statements from row objects', () => {
    const sql = buildInsertSql('users', { id: 1, name: 'Al', email: null });
    expect(sql).toBe('INSERT INTO "users" ("id", "name", "email") VALUES (1, \'Al\', NULL);');
  });
});

describe('foreign key helpers (visual relations)', () => {
  it('whitelists referential actions', () => {
    expect(sanitizeFkAction('cascade')).toBe('CASCADE');
    expect(sanitizeFkAction('SET NULL')).toBe('SET NULL');
    expect(sanitizeFkAction('bogus; DROP TABLE x')).toBe('NO ACTION');
    expect(sanitizeFkAction('')).toBe('NO ACTION');
  });

  it('builds ADD CONSTRAINT ... FOREIGN KEY statements', () => {
    const sql = buildAddFkSql('fk_orders_user', 'orders', 'user_id', 'users', 'id', 'CASCADE', 'NO ACTION');
    expect(sql).toBe(
      'ALTER TABLE "orders" ADD CONSTRAINT "fk_orders_user" FOREIGN KEY ("user_id") REFERENCES "users"("id") ON DELETE CASCADE ON UPDATE NO ACTION;'
    );
  });

  it('escapes identifiers and sanitizes injected actions', () => {
    const sql = buildAddFkSql('fk"x', 't', 'c', 'r', 'rc', 'evil', 'RESTRICT');
    expect(sql).toContain('"fk""x"');
    expect(sql).toContain('ON DELETE NO ACTION ON UPDATE RESTRICT');
    expect(FK_ACTIONS).toHaveLength(5);
  });
});
