// SPDX-License-Identifier: GPL-3.0-only

#include "Hyprland.hpp"
#include "../hyprland/src/managers/SeatManager.hpp"
#include "../hyprland/src/protocols/types/DataDevice.hpp"
#include <string>
#include <vector>

namespace YuOs {
    class CSelectionSource final : public IDataSource {
      public:
        CSelectionSource(YuOsSelection* selection, std::vector<std::string> mimes);
        ~CSelectionSource() override;

        std::vector<std::string> mimes() override;
        void                     send(const std::string& mime, Hyprutils::OS::CFileDescriptor descriptor) override;
        void                     accepted(const std::string& mime) override;
        void                     cancelled() override;
        void                     error(uint32_t code, const std::string& message) override;

      private:
        YuOsSelection*           m_selection = nullptr;
        std::vector<std::string> m_mimes;
    };

    static SP<CSelectionSource>& selectionOwner() {
        static SP<CSelectionSource> selection;
        return selection;
    }

    CSelectionSource::CSelectionSource(YuOsSelection* selection, std::vector<std::string> mimes) : m_selection(selection), m_mimes(std::move(mimes)) {
        ;
    }

    CSelectionSource::~CSelectionSource() {
        m_events.destroy.emit();
        yuos_selection_close(m_selection);
    }

    std::vector<std::string> CSelectionSource::mimes() {
        return m_mimes;
    }

    void CSelectionSource::send(const std::string& mime, Hyprutils::OS::CFileDescriptor descriptor) {
        yuos_selection_send(m_selection, reinterpret_cast<const uint8_t*>(mime.data()), mime.size(), descriptor.get());
    }

    void CSelectionSource::accepted(const std::string&) {
        ;
    }

    void CSelectionSource::cancelled() {
        ;
    }

    void CSelectionSource::error(uint32_t, const std::string&) {
        ;
    }

    bool installSelection(YuOsSelection* selection) {
        if (!selection || !g_pSeatManager)
            return false;
        const auto count = yuos_selection_count(selection);
        if (count == 0 || count > 64)
            return false;
        std::vector<std::string> mimes;
        mimes.reserve(count);
        for (size_t index = 0; index < count; ++index) {
            const auto* mime = yuos_selection_mime(selection, index);
            if (!mime)
                return false;
            mimes.emplace_back(mime);
        }
        auto source      = makeShared<CSelectionSource>(selection, std::move(mimes));
        auto previous    = std::move(selectionOwner());
        selectionOwner() = source;
        g_pSeatManager->setCurrentSelection(source);
        return true;
    }

    void clearSelection() {
        selectionOwner().reset();
    }
}
