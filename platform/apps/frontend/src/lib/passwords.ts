// apps/frontend/src/lib/passwords.ts
import bcrypt from "bcrypt";

export async function hashPassword(plain: string) {
  return bcrypt.hash(plain, 10);
}
