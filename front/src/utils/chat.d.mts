export interface ChatFragment { text: string; emoteId?: string | null; }
export interface ChatMessage { [key: string]: any; message: string; fragments: ChatFragment[]; }
export function emoteFragments(message: string, emotes?: string): ChatFragment[];
export function parseChatFrame(frame: string): ChatMessage[];
