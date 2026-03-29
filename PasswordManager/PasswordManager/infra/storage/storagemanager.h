#ifndef STORAGEMANAGER_H
#define STORAGEMANAGER_H

#include "../../core/ports/istoragemanager.h"

#include <filesystem>
#include <system_error>
#include <iostream>

namespace fs = std::filesystem;

class StorageManager : IStorageManager
{
public:
	void Save(const std::string& path) override;

	std::expected<std::string, std::error_code> Load(const std::string& path) const override;

private:
	bool FileValid(const std::string& path, std::error_code& ec) const;
};

#endif // STORAGEMANAGER_H