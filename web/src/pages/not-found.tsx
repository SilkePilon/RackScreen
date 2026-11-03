import { Link } from "react-router"
import { CompassIcon } from "lucide-react"

import { PageBody } from "@/components/page"
import { Button } from "@/components/ui/button"
import { Empty, EmptyContent, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle } from "@/components/ui/empty"

export default function NotFound() {
  return (
    <PageBody>
      <Empty className="border">
        <EmptyHeader>
          <EmptyMedia variant="icon">
            <CompassIcon />
          </EmptyMedia>
          <EmptyTitle>No page here</EmptyTitle>
          <EmptyDescription>The address does not match any page. Press / to search.</EmptyDescription>
        </EmptyHeader>
        <EmptyContent>
          <Button asChild variant="outline">
            <Link to="/">Back to the overview</Link>
          </Button>
        </EmptyContent>
      </Empty>
    </PageBody>
  )
}
