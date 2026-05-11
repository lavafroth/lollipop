import Quickshell
import Quickshell.Io
import QtQuick

import Quickshell
import QtQuick

ShellRoot {

    FileView {
        id: shmFile
        path: Qt.resolvedUrl("/dev/shm/lollipop.shm")
        watchChanges: true

        onFileChanged: {
            this.reload()
        }
    }

    SystemPalette {
        id: sysPalette
        colorGroup: SystemPalette.Active
    }

    PanelWindow {
        anchors {
            top: true
            right: true
        }

        implicitHeight: 40
        implicitWidth: statusText.implicitWidth + 20 // grow as needed

        // Floating style: transparent background with a rounded rectangle
        color: "transparent"

        Rectangle {
            anchors.fill: parent
            radius: 8
            color: sysPalette.window
            border.color: sysPalette.highlight
            border.width: 3

            Text {
                id: statusText
                anchors.centerIn: parent
                padding: 10
                color: sysPalette.windowText
                text: shmFile.text()
                font.pixelSize: 14

            }
        }
    }
}


