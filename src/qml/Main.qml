import QtQuick
import Qt.labs.platform as Platform
import io.github.escucha

Item {
    id: root
    visible: false
    width: 0
    height: 0

    property var backend: EscuchaBackend {}

    function tooltipText() {
        var parts = [backend.statusText]
        if (backend.deviceName) {
            parts.push(backend.deviceName)
        }
        if (backend.statusDetail) {
            parts.push(backend.statusDetail)
        }
        return parts.join(" - ")
    }

    function notifyStatus() {
        var body = backend.statusDetail
        if (!body && backend.deviceName) {
            body = backend.deviceName
        }
        tray.showMessage("Escucha - " + backend.statusText, body, Platform.SystemTrayIcon.Information, 2500)
    }

    Platform.SystemTrayIcon {
        id: tray
        visible: true
        icon.name: backend.statusIconName || "io.github.escucha"
        tooltip: root.tooltipText()

        menu: Platform.Menu {
            title: "Escucha"

            Platform.MenuItem {
                text: "Status: " + backend.statusText
                enabled: false
            }

            Platform.MenuItem {
                text: backend.deviceName ? ("Device: " + backend.deviceName) : "Device: Detecting..."
                enabled: false
            }

            Platform.MenuItem {
                text: backend.statusDetail ? backend.statusDetail : "Hold Right Ctrl to speak"
                enabled: false
            }

            Platform.MenuSeparator {}

            Platform.MenuItem {
                text: "Last: " + backend.transcription
                enabled: false
            }

            Platform.MenuSeparator {}

            Platform.MenuItem {
                text: "Fix Input Permissions"
                visible: backend.showFixButton
                onTriggered: backend.fixPermissions()
            }

            Platform.MenuItem {
                text: "Fix Paste Setup"
                visible: backend.showPasteFixButton
                onTriggered: backend.fixPasteSetup()
            }

            Platform.MenuItem {
                text: "Quit Escucha"
                onTriggered: {
                    backend.requestShutdown()
                    Qt.quit()
                }
            }
        }

        onActivated: function(reason) {
            if (reason === Platform.SystemTrayIcon.Trigger) {
                root.notifyStatus()
            }
        }
    }

    Connections {
        target: backend

        function onErrorOccurred(message) {
            tray.showMessage("Escucha Error", message, Platform.SystemTrayIcon.Critical, 5000)
        }
    }
}
