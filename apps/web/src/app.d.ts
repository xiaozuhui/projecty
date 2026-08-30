declare global {
  namespace App {
    interface Locals {
      user?: { id: string; account: string; displayName: string; systemRole: 'super_admin' | 'user' };
    }
  }
}
export {};
