// A fake backend for the state modules' tests (PLAN.md H13). `api.ts` runs
// as it does in the app, calling `commands.*`; Tauri's IPC mocks
// (`@tauri-apps/api/mocks`) send each command here, by the name its
// wrapper in `generated/commands.ts` has, and the test answers it. Events
// go through Tauri's own event plumbing to the stores' `on` listeners.
//
// A command the test didn't answer rejects, and `setup.ts` fails the test,
// so a store that starts calling something new shows up.

import { emit } from "@tauri-apps/api/event";
import { mockIPC } from "@tauri-apps/api/mocks";
import type { on } from "$lib/api";
import { commands } from "$lib/generated/commands";

type Commands = typeof commands;
export type Command = keyof Commands;
/** A command's arguments, as its wrapper takes them. */
export type Args<C extends Command> = Parameters<Commands[C]>[0];
/** What a command resolves to. */
export type Reply<C extends Command> = Awaited<ReturnType<Commands[C]>>;
type Answer<C extends Command> = (args: Args<C>) => Reply<C> | Promise<Reply<C>>;

/** The backend's events and their payloads, as `api.ts`'s `on` types them. */
export type Event = Parameters<typeof on>[0];
export type Payload<E extends Event> = Parameters<Parameters<typeof on<E>>[1]>[0];

/** "settings_save" → "settingsSave", as `bindings.rs` names the wrappers. */
const wrapperName = (command: string) => command.replace(/_([a-z0-9])/g, (_, c: string) => c.toUpperCase());

export class FakeBackend {
  /** Every command invoked, in order, with its arguments. */
  readonly calls: { command: Command; args: unknown }[] = [];
  /** Commands invoked that the test didn't answer. */
  readonly unanswered: string[] = [];
  #answers = new Map<Command, Answer<Command>>();

  constructor() {
    mockIPC(
      (command, args) => {
        const name = wrapperName(command) as Command;
        if (!(name in commands)) throw new Error(`${command} isn't a generated command`);
        this.calls.push({ command: name, args: structuredClone(args) });
        const answer = this.#answers.get(name);
        if (!answer) {
          this.unanswered.push(name);
          throw new Error(`no answer for ${name}`);
        }
        // A copy, as the real IPC deserializes one: a store can't change the test's fixture.
        return Promise.resolve(answer(args as Args<Command>)).then((reply) => structuredClone(reply));
      },
      { shouldMockEvents: true },
    );
  }

  /** Answers `command` with `reply`, or with what `answer` returns for its arguments. */
  answer<C extends Command>(command: C, reply: Answer<C> | Reply<C>): this {
    const answer = typeof reply === "function" ? reply : () => reply;
    this.#answers.set(command, answer as Answer<Command>);
    return this;
  }

  /** Makes `command` fail with `error`, the text a Rust `Err(String)` rejects with (a coded error's JSON). */
  fail(command: Command, error: string): this {
    this.#answers.set(command, () => Promise.reject(error));
    return this;
  }

  /** The arguments of each call of `command`, in order. */
  argsOf<C extends Command>(command: C): Args<C>[] {
    return this.calls.filter((call) => call.command === command).map((call) => call.args as Args<C>);
  }

  /** Sends a backend event to whoever listens. */
  emit<E extends Event>(event: E, payload: Payload<E>): Promise<void> {
    return emit(event, structuredClone(payload));
  }
}

/** A coded error's text, as `crate::coded` writes it, for errors no fixture records. */
export const coded = (code: string, params: Record<string, string | number> = {}, message = code) =>
  JSON.stringify({ code, params, message });

let current: FakeBackend | null = null;

/** A fake backend for this test; install it before importing the stores. */
export function fakeBackend(): FakeBackend {
  current = new FakeBackend();
  return current;
}

/** Ends the test's backend (`setup.ts`): the commands no one answered. */
export function endBackend(): string[] {
  const unanswered = current?.unanswered ?? [];
  current = null;
  return unanswered;
}

/** Lets pending promises (replies, listeners being registered) run. */
export const settle = () => new Promise<void>((resolve) => setTimeout(resolve, 0));
