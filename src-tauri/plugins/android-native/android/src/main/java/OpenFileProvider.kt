package app.moodledesk.androidnative

import androidx.core.content.FileProvider

// Its own class, so the manifest merger does not mix it up with the FileProvider the app's generated manifest already declares
class OpenFileProvider : FileProvider()
