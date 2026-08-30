Output assets/demo.gif
Set Shell "zsh"
Set FontSize 28
Set Width 1100
Set Height 500
Set Padding 24
Set Framerate 30
Set Theme "Catppuccin Mocha"
Set WindowBar Colorful
Set BorderRadius 8
Set TypingSpeed 45ms
Set CursorBlink false

Hide
Type "clear" Enter
Sleep 500ms
Show

# 1 - version check
Type "gbat --version" Sleep 800ms Enter
Sleep 1200ms

# 2 - main battery read
Type "gbat" Sleep 600ms Enter
Sleep 1500ms

# 3 - Raycast script path
Type "./raycast/mouse-battery.sh" Sleep 600ms Enter
Sleep 1500ms

# 4 - stdout is scriptable
Type "echo $(gbat)" Sleep 600ms Enter
Sleep 1500ms

Sleep 2s
