/*
 * Simple C API wrapper implementation for mdflib
 */

#include "mdf_c_api.h"
#include "mdf/mdfreader.h"
#include "mdf/canmessage.h"
#include "mdf/ichannelgroup.h"
#include "mdf/idatagroup.h"
#include "mdf/ichannelobserver.h"

#include <memory>
#include <string>
#include <vector>
#include <cstring>
#include <thread>

// Global error message buffer
static std::string g_last_error;

const char* mdf_get_error_message() {
    return g_last_error.c_str();
}

// Reader wrapper structure
struct MdfReaderWrapper {
    std::unique_ptr<mdf::MdfReader> reader;
    std::string filename;
};

// Iterator wrapper structure
struct MdfIteratorWrapper {
    mdf::MdfReader* reader;  // Non-owning pointer
    mdf::ChannelObserverList observers;
    size_t current_sample;
    uint64_t total_samples;
    bool has_can_data;
};

extern "C" {

MdfReaderHandle mdf_open(const char* filename, MdfError* error) {
    if (!filename) {
        g_last_error = "Filename is NULL";
        if (error) *error = MDF_ERROR_NULL_HANDLE;
        return nullptr;
    }

    try {
        // Check if file is MDF
        if (!mdf::IsMdfFile(filename)) {
            g_last_error = "File is not a valid MDF file";
            if (error) *error = MDF_ERROR_NOT_MDF_FILE;
            return nullptr;
        }

        // Create reader
        auto wrapper = new MdfReaderWrapper();
        wrapper->filename = filename;
        wrapper->reader = std::make_unique<mdf::MdfReader>(filename);

        // Open and read header
        if (!wrapper->reader->Open()) {
            g_last_error = "Failed to open MDF file";
            delete wrapper;
            if (error) *error = MDF_ERROR_OPEN_FAILED;
            return nullptr;
        }

        if (!wrapper->reader->ReadEverythingButData()) {
            g_last_error = "Failed to read MDF file structure";
            wrapper->reader->Close();
            delete wrapper;
            if (error) *error = MDF_ERROR_READ_FAILED;
            return nullptr;
        }

        if (error) *error = MDF_OK;
        return wrapper;

    } catch (const std::exception& e) {
        g_last_error = std::string("Exception: ") + e.what();
        if (error) *error = MDF_ERROR_OPEN_FAILED;
        return nullptr;
    }
}

void mdf_close(MdfReaderHandle reader) {
    if (!reader) return;

    auto* wrapper = static_cast<MdfReaderWrapper*>(reader);
    if (wrapper->reader && wrapper->reader->IsOpen()) {
        wrapper->reader->Close();
    }
    delete wrapper;
}

MdfIteratorHandle mdf_create_can_iterator(MdfReaderHandle reader, MdfError* error) {
    if (!reader) {
        g_last_error = "Reader handle is NULL";
        if (error) *error = MDF_ERROR_NULL_HANDLE;
        return nullptr;
    }

    auto* wrapper = static_cast<MdfReaderWrapper*>(reader);

    try {
        auto* iter = new MdfIteratorWrapper();
        iter->reader = wrapper->reader.get();
        iter->current_sample = 0;
        iter->total_samples = 0;
        iter->has_can_data = false;

        // Find CAN data groups and create observers
        const auto* file = wrapper->reader->GetFile();
        if (!file) {
            g_last_error = "Failed to get MDF file object";
            delete iter;
            if (error) *error = MDF_ERROR_READ_FAILED;
            return nullptr;
        }

        // Get data groups
        mdf::DataGroupList dg_list;
        file->DataGroups(dg_list);

        // Iterate through data groups to find CAN data
        for (auto* dg : dg_list) {
            if (!dg) continue;

            // Check channel groups for CAN data
            auto cg_list = dg->ChannelGroups();
            for (auto* cg : cg_list) {
                if (!cg) continue;

                // Look for CAN channels (typically named CAN_DataFrame, CAN_ID, etc.)
                auto cn_list = cg->Channels();
                bool has_can_channels = false;

                for (auto* cn : cn_list) {
                    if (cn) {
                        std::string name = cn->Name();
                        // Check if this is a CAN-related channel
                        if (name.find("CAN") != std::string::npos ||
                            name.find("BusChannel") != std::string::npos) {
                            has_can_channels = true;
                            break;
                        }
                    }
                }

                if (has_can_channels) {
                    // Create observers for this channel group
                    mdf::CreateChannelObserverForChannelGroup(*dg, *cg, iter->observers);
                    iter->has_can_data = true;

                    // Get sample count
                    iter->total_samples += cg->NofSamples();
                }
            }

            // Read data for this data group if it has CAN data
            if (iter->has_can_data) {
                wrapper->reader->ReadData(*dg);
            }
        }

        if (!iter->has_can_data || iter->observers.empty()) {
            g_last_error = "No CAN data found in MDF file";
            delete iter;
            if (error) *error = MDF_ERROR_NO_CAN_DATA;
            return nullptr;
        }

        if (error) *error = MDF_OK;
        return iter;

    } catch (const std::exception& e) {
        g_last_error = std::string("Exception: ") + e.what();
        if (error) *error = MDF_ERROR_READ_FAILED;
        return nullptr;
    }
}

MdfError mdf_iterator_next(MdfIteratorHandle iterator, MdfCanFrame* frame) {
    if (!iterator) {
        g_last_error = "Iterator handle is NULL";
        return MDF_ERROR_NULL_HANDLE;
    }

    if (!frame) {
        g_last_error = "Frame pointer is NULL";
        return MDF_ERROR_NULL_HANDLE;
    }

    auto* iter = static_cast<MdfIteratorWrapper*>(iterator);

    // TODO: Implement actual frame iteration
    // This is a simplified stub - full implementation would:
    // 1. Iterate through channel observers
    // 2. Extract CAN message data from samples
    // 3. Parse CAN ID, data, flags from channel values
    // 4. Convert timestamps to nanoseconds

    if (iter->current_sample >= iter->total_samples) {
        return MDF_ERROR_END_OF_DATA;
    }

    // Stub: Return dummy data for now
    // TODO: Replace with actual mdflib data extraction
    memset(frame, 0, sizeof(MdfCanFrame));
    frame->timestamp_ns = 0;
    frame->channel = 0;
    frame->can_id = 0x123;
    frame->data_length = 8;
    frame->is_extended = 0;
    frame->is_fd = 0;
    frame->is_error_frame = 0;
    frame->is_remote_frame = 0;

    iter->current_sample++;

    return MDF_OK;
}

void mdf_iterator_free(MdfIteratorHandle iterator) {
    if (!iterator) return;
    auto* iter = static_cast<MdfIteratorWrapper*>(iterator);
    delete iter;
}

} // extern "C"
