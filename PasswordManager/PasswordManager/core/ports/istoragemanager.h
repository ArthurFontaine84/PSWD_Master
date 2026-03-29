#ifndef ISTORAGEMANAGER_H
#define ISTORAGEMANAGER_H

#include <string>
#include <system_error>
#include <expected>

class IStorageManager
{
public:
	virtual ~IStorageManager() = default;

	virtual void Save(const std::string& path) = 0;
	virtual std::expected<std::string, std::error_code> Load(const std::string& path) const = 0;
};
#endif // ISTORAGEMANAGER_H
