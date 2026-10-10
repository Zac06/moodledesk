# Privacy

MoodleDesk runs on your computer. It has no accounts, no analytics, no crash reporting and no servers of its own, and nothing is sent to the developer. We do not collect your data.

**What it stores on your computer**
- Your Moodle login token, in the system keychain.
- The files you download, in the folder you choose, and a list of those downloads in the app's data folder.
- Your settings (theme, download mode, download folder), in the app's local storage.

**Who it talks to**
- **Your Moodle site.** Everything you see (courses, files, grades, dates) and your login go there. With password login your username and password are sent to it. Only `https://` sites are accepted. Your site sees the requests as coming from a Moodle mobile app and may log them under its own privacy policy.
- **GitHub (`api.github.com`).** The app asks it for the latest version. GitHub can see your IP address. The project website does the same to show the download buttons.
- **Images in course text** are shown only if they come from your Moodle site, so a teacher's pasted picture from another website is not loaded and cannot reveal your IP address. Cover images use the address your Moodle site provides.
- **Links** you click open in your browser. After that, the other website's own policy applies.

Questions or problems: open an issue at https://github.com/zac06/moodledesk/issues.
