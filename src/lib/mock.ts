export async function mockInvoke(cmd: string, _args?: Record<string, unknown>): Promise<unknown> {
	throw new Error(`no mock for ${cmd}`);
}
