declare module '@tauri-apps/plugin-dialog' {
  type OpenDialogOptions = {
    directory?: boolean
    multiple?: boolean
    title?: string
  }

  type SaveDialogOptions = {
    title?: string
    defaultPath?: string
    filters?: { name: string; extensions: string[] }[]
  }

  export function open(options?: OpenDialogOptions): Promise<string | string[] | null>
  export function save(options?: SaveDialogOptions): Promise<string | null>
}
