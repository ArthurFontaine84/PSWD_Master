#include "storagemanager.h"
#include <fstream>

void StorageManager::Save(const std::string& path)
{

}

std::expected<std::string, std::error_code> StorageManager::Load(const std::string& path) const
{
    std::error_code ec; // TODO : voir si init la var et passage en ref

    if (!FileValid(path, ec))
        return std::unexpected(std::make_error_code(std::errc::no_such_file_or_directory));

    std::ifstream file(path, std::ios::binary);

    if (!file.is_open()) {
        return std::unexpected(std::make_error_code(std::errc::io_error));
    }

    std::ostringstream contentStream;
    contentStream << file.rdbuf();
    std::string encryptedContent = contentStream.str();

    file.close();
    return encryptedContent;
}

bool StorageManager::FileValid(const std::string& path, std::error_code& ec) const
{
    if (path.empty())
        return false;

    bool valid = fs::exists(path, ec) && fs::is_regular_file(path, ec);

    return valid;
}
