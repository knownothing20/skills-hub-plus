Set WshShell = CreateObject("WScript.Shell")
Set fso = CreateObject("Scripting.FileSystemObject")
scriptDir = fso.GetParentFolderName(WScript.ScriptFullName)
repoDir = fso.GetParentFolderName(scriptDir)

exeRelease = repoDir & "\src-tauri\target\release\skills-hub.exe"
exeDist = repoDir & "\dist-app\skills-hub.exe"

If fso.FileExists(exeRelease) Then
    WshShell.Run """" & exeRelease & """ --minimized", 0, False
ElseIf fso.FileExists(exeDist) Then
    WshShell.Run """" & exeDist & """ --minimized", 0, False
Else
    WshShell.Run "cmd /c cd /d """ & repoDir & """ && npm run tauri:dev -- -- --minimized", 0, False
End If
